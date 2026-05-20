//! Checagem propriamente dita. Percorre cada fn body computando tipos e
//! acumulando erros.

use indexmap::IndexMap;
use smol_str::SmolStr;
use vex_ast::{BinOp, UnaryOp};
use vex_hir::{
    DefId, DefKind, HirBlock, HirExpr, HirImpl, HirItem, HirModule, HirStmt,
};

use crate::env::Env;
use crate::ty::{builtin_signature, lower_hir_type, unify, Ty};

pub type Span = std::ops::Range<usize>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum TypeError {
    #[error("tipos incompatíveis: esperado `{expected}`, encontrado `{found}`")]
    Mismatch { expected: String, found: String, span: Span },

    #[error("operador `{op}` não pode ser aplicado a `{lhs}` e `{rhs}`")]
    BadBinOp { op: &'static str, lhs: String, rhs: String, span: Span },

    #[error("operador unário `{op}` não pode ser aplicado a `{ty}`")]
    BadUnaryOp { op: &'static str, ty: String, span: Span },

    #[error("chamada com aridade errada: esperava {expected} argumento(s), recebeu {found}")]
    BadArity { expected: usize, found: usize, span: Span },

    #[error("`{name}` não é chamável")]
    NotCallable { name: String, span: Span },

    #[error("campo `{field}` não existe na struct `{struct_name}`")]
    UnknownField { struct_name: String, field: SmolStr, span: Span },

    #[error("método `{method}` não existe para o tipo receptor")]
    UnknownMethod { method: SmolStr, span: Span },

    #[error("retorno do tipo errado: função declara `{expected}`, mas retorna `{found}`")]
    BadReturn { expected: String, found: String, span: Span },

    #[error("condição de `{kind}` precisa ser bool, recebeu `{found}`")]
    NonBoolCond { kind: &'static str, found: String, span: Span },

    #[error("índice de array precisa ser int, recebeu `{found}`")]
    NonIntIndex { found: String, span: Span },

    #[error("não é possível indexar valor do tipo `{ty}`")]
    NotIndexable { ty: String, span: Span },

    #[error("não é possível acessar campo de valor do tipo `{ty}`")]
    NoFields { ty: String, span: Span },

    #[error("struct literal `{name}` está faltando campo `{field}`")]
    MissingField { name: String, field: SmolStr, span: Span },

    #[error("struct literal `{name}` tem campo desconhecido `{field}`")]
    ExtraField { name: String, field: SmolStr, span: Span },
}

impl TypeError {
    pub fn span(&self) -> &Span {
        match self {
            TypeError::Mismatch { span, .. }
            | TypeError::BadBinOp { span, .. }
            | TypeError::BadUnaryOp { span, .. }
            | TypeError::BadArity { span, .. }
            | TypeError::NotCallable { span, .. }
            | TypeError::UnknownField { span, .. }
            | TypeError::UnknownMethod { span, .. }
            | TypeError::BadReturn { span, .. }
            | TypeError::NonBoolCond { span, .. }
            | TypeError::NonIntIndex { span, .. }
            | TypeError::NotIndexable { span, .. }
            | TypeError::NoFields { span, .. }
            | TypeError::MissingField { span, .. }
            | TypeError::ExtraField { span, .. } => span,
        }
    }
}

/// Roda type-check sobre o módulo HIR. Retorna lista de erros (vazia = OK).
pub fn check_module(module: &HirModule) -> Vec<TypeError> {
    let env = Env::build(module);
    let mut checker = Checker {
        env: &env,
        locals: Vec::new(),
        self_ty: None,
        expected_ret: None,
        errors: Vec::new(),
    };

    for item in &module.items {
        match item {
            HirItem::Fn(f) => {
                let sig = checker.env.fns.get(&f.id).cloned().unwrap();
                checker.check_fn_body(f, &sig, None);
            }
            HirItem::Impl(im) => {
                checker.check_impl(im);
            }
            HirItem::Const(c) => {
                let expected = lower_hir_type(&c.ty, None);
                let actual = checker.infer_expr(&c.value);
                checker.expect_type(&expected, &actual, c.value.span());
            }
            HirItem::Struct(_) => { /* fields já lowered no Env */ }
        }
    }

    checker.errors
}

struct Checker<'h> {
    env: &'h Env<'h>,
    locals: Vec<IndexMap<DefId, Ty>>,
    self_ty: Option<Ty>,
    /// Tipo de retorno esperado da fn atual. `None` fora de fn body.
    expected_ret: Option<Ty>,
    errors: Vec<TypeError>,
}

impl<'h> Checker<'h> {
    // ── helpers de escopo ─────────────────────────────────────────────

    fn push_scope(&mut self) { self.locals.push(IndexMap::new()); }
    fn pop_scope(&mut self)  { self.locals.pop(); }
    fn declare(&mut self, id: DefId, ty: Ty) {
        if let Some(top) = self.locals.last_mut() { top.insert(id, ty); }
    }
    fn lookup(&self, id: DefId) -> Option<Ty> {
        for scope in self.locals.iter().rev() {
            if let Some(t) = scope.get(&id) { return Some(t.clone()); }
        }
        None
    }

    // ── erros ─────────────────────────────────────────────────────────

    fn expect_type(&mut self, expected: &Ty, found: &Ty, span: Span) {
        if !unify(expected, found) {
            self.errors.push(TypeError::Mismatch {
                expected: expected.to_string(),
                found:    found.to_string(),
                span,
            });
        }
    }

    // ── checagem de itens ────────────────────────────────────────────

    fn check_impl(&mut self, im: &HirImpl) {
        let self_ty = Ty::Struct(im.target);
        let prev_self = self.self_ty.replace(self_ty.clone());
        for method in &im.methods {
            let sig = self.env.methods
                .get(&(im.target, method.name.clone()))
                .cloned()
                .unwrap_or_else(|| crate::env::FnSig {
                    params: vec![],
                    ret: Ty::Error,
                });
            self.check_fn_body(method, &sig, Some(&self_ty));
        }
        self.self_ty = prev_self;
    }

    fn check_fn_body(&mut self, f: &vex_hir::HirFn, sig: &crate::env::FnSig, self_ty: Option<&Ty>) {
        let prev_self = self.self_ty.clone();
        if self_ty.is_some() { self.self_ty = self_ty.cloned(); }
        let prev_ret = self.expected_ret.replace(sig.ret.clone());

        self.push_scope();
        for (param, ty) in f.params.iter().zip(sig.params.iter()) {
            self.declare(param.id, ty.clone());
        }
        let actual_ret = self.check_block(&f.body);
        // Função sem return explícito: bloco vazio ou que cai no fim
        // implica retorno `void`. Compatível somente se sig.ret == void.
        if actual_ret.is_none() && !unify(&sig.ret, &Ty::Void) {
            self.errors.push(TypeError::BadReturn {
                expected: sig.ret.to_string(),
                found: "void".into(),
                span: f.body.span.clone(),
            });
        }
        self.pop_scope();

        self.expected_ret = prev_ret;
        self.self_ty = prev_self;
    }

    /// Checa um bloco. Retorna `Some(ty)` se algum return explícito foi
    /// encontrado (com o tipo retornado), `None` caso contrário.
    fn check_block(&mut self, b: &HirBlock) -> Option<Ty> {
        self.push_scope();
        let mut returned: Option<Ty> = None;
        for stmt in &b.stmts {
            if let Some(t) = self.check_stmt(stmt) {
                returned = Some(t);
            }
        }
        self.pop_scope();
        returned
    }

    fn check_stmt(&mut self, s: &HirStmt) -> Option<Ty> {
        match s {
            HirStmt::Let { id, type_ann, value, span, .. } => {
                let expected = type_ann.as_ref().map(|t| lower_hir_type(t, self.self_ty.as_ref()));
                let inferred = self.infer_expr(value);
                if let Some(ref e) = expected {
                    self.expect_type(e, &inferred, span.clone());
                }
                let ty = expected.unwrap_or(inferred);
                self.declare(*id, ty);
                None
            }
            HirStmt::Return(opt, span) => {
                let ty = match opt {
                    Some(e) => self.infer_expr(e),
                    None    => Ty::Void,
                };
                self.check_return_against_current_fn(&ty, span.clone());
                Some(ty)
            }
            HirStmt::If { cond, then_body, else_body, .. } => {
                let ct = self.infer_expr(cond);
                if !unify(&ct, &Ty::Bool) {
                    self.errors.push(TypeError::NonBoolCond {
                        kind: "if", found: ct.to_string(), span: cond.span(),
                    });
                }
                let t = self.check_block(then_body);
                let e = else_body.as_ref().and_then(|b| self.check_block(b));
                // Se ambos retornaram, retornam o mesmo? Por enquanto não exigimos
                // (validação de exhaustive return fica para typeck avançado).
                t.or(e)
            }
            HirStmt::While { cond, body, .. } => {
                let ct = self.infer_expr(cond);
                if !unify(&ct, &Ty::Bool) {
                    self.errors.push(TypeError::NonBoolCond {
                        kind: "while", found: ct.to_string(), span: cond.span(),
                    });
                }
                self.check_block(body);
                None
            }
            HirStmt::For { var_id, iter, body, .. } => {
                let iter_ty = self.infer_expr(iter);
                let elem = match iter_ty {
                    Ty::Array(inner) => *inner,
                    Ty::Error => Ty::Error,
                    _ => {
                        // Sem suporte a ranges como iter ainda.
                        Ty::Error
                    }
                };
                self.push_scope();
                self.declare(*var_id, elem);
                self.check_block(body);
                self.pop_scope();
                None
            }
            HirStmt::Break(_) | HirStmt::Continue(_) => None,
            HirStmt::Expr(e) => { self.infer_expr(e); None }
        }
    }

    fn check_return_against_current_fn(&mut self, ty: &Ty, span: Span) {
        if let Some(expected) = self.expected_ret.clone() {
            if !unify(&expected, ty) {
                self.errors.push(TypeError::BadReturn {
                    expected: expected.to_string(),
                    found: ty.to_string(),
                    span,
                });
            }
        }
    }

    // ── inferência de expressões ─────────────────────────────────────

    fn infer_expr(&mut self, e: &HirExpr) -> Ty {
        match e {
            HirExpr::Int(_, _)   => Ty::Int,
            HirExpr::Float(_, _) => Ty::Float,
            HirExpr::Str(_, _)   => Ty::Str,
            HirExpr::Bool(_, _)  => Ty::Bool,

            HirExpr::Name { id, .. } => {
                // local? param?
                if let Some(t) = self.lookup(*id) { return t; }
                // fn → tipo "fn(...) -> R" simplificado: retornamos Ty::Error
                // por enquanto, mas Call trata fn especialmente via id.
                let def = self.env.module.def(*id);
                match def.kind {
                    DefKind::Const => {
                        // const já foi checada e tem tipo declarado.
                        // Lookup pelo HIR const.
                        self.const_type(*id).unwrap_or(Ty::Error)
                    }
                    _ => Ty::Error,
                }
            }
            HirExpr::SelfRef(_) => self.self_ty.clone().unwrap_or(Ty::Error),
            HirExpr::Builtin { .. } => Ty::Any,

            HirExpr::BinOp { op, left, right, span } => {
                let l = self.infer_expr(left);
                let r = self.infer_expr(right);
                self.check_binop(*op, &l, &r, span.clone())
            }
            HirExpr::UnaryOp { op, val, span } => {
                let t = self.infer_expr(val);
                match op {
                    UnaryOp::Neg if t.is_numeric() || t.is_error() => t,
                    UnaryOp::Not if unify(&t, &Ty::Bool) || t.is_error() => Ty::Bool,
                    _ => {
                        self.errors.push(TypeError::BadUnaryOp {
                            op: match op { UnaryOp::Neg => "-", UnaryOp::Not => "!" },
                            ty: t.to_string(),
                            span: span.clone(),
                        });
                        Ty::Error
                    }
                }
            }

            HirExpr::Call { callee, args, span } => self.infer_call(callee, args, span.clone()),

            HirExpr::MethodCall { receiver, name, args, span } => {
                let recv_ty = self.infer_expr(receiver);
                let struct_id = match &recv_ty {
                    Ty::Struct(id) => Some(*id),
                    Ty::Ref { inner, .. } => match &**inner {
                        Ty::Struct(id) => Some(*id),
                        _ => None,
                    },
                    Ty::Error => return Ty::Error,
                    _ => None,
                };
                let Some(struct_id) = struct_id else {
                    self.errors.push(TypeError::UnknownMethod {
                        method: name.clone(),
                        span: span.clone(),
                    });
                    return Ty::Error;
                };
                let key = (struct_id, name.clone());
                let Some(sig) = self.env.methods.get(&key).cloned() else {
                    self.errors.push(TypeError::UnknownMethod {
                        method: name.clone(),
                        span: span.clone(),
                    });
                    return Ty::Error;
                };
                // Pula `self` (primeiro param) na checagem de aridade.
                let expected_params = sig.params.iter().skip(1).cloned().collect::<Vec<_>>();
                self.check_call_args(&expected_params, args, span.clone());
                sig.ret
            }

            HirExpr::FieldAccess { obj, field, span } => {
                let t = self.infer_expr(obj);
                let struct_id = match &t {
                    Ty::Struct(id) => Some(*id),
                    Ty::Ref { inner, .. } => match &**inner {
                        Ty::Struct(id) => Some(*id),
                        _ => None,
                    },
                    Ty::Error => return Ty::Error,
                    _ => {
                        self.errors.push(TypeError::NoFields {
                            ty: t.to_string(), span: span.clone(),
                        });
                        return Ty::Error;
                    }
                };
                let Some(struct_id) = struct_id else { return Ty::Error };
                let fields = self.env.structs.get(&struct_id);
                let struct_name = self.env.module.def(struct_id).name.clone();
                match fields.and_then(|m| m.get(field)) {
                    Some(t) => t.clone(),
                    None => {
                        self.errors.push(TypeError::UnknownField {
                            struct_name: struct_name.to_string(),
                            field: field.clone(),
                            span: span.clone(),
                        });
                        Ty::Error
                    }
                }
            }

            HirExpr::Index { obj, idx, span } => {
                let t = self.infer_expr(obj);
                let i = self.infer_expr(idx);
                if !unify(&i, &Ty::Int) {
                    self.errors.push(TypeError::NonIntIndex {
                        found: i.to_string(), span: span.clone(),
                    });
                }
                match t {
                    Ty::Array(inner) => *inner,
                    Ty::Error => Ty::Error,
                    other => {
                        self.errors.push(TypeError::NotIndexable {
                            ty: other.to_string(), span: span.clone(),
                        });
                        Ty::Error
                    }
                }
            }

            HirExpr::Array(items, _span) => {
                let first = items.first().map(|e| self.infer_expr(e));
                for rest in items.iter().skip(1) {
                    let t = self.infer_expr(rest);
                    if let Some(ref f) = first {
                        self.expect_type(f, &t, rest.span());
                    }
                }
                Ty::Array(Box::new(first.unwrap_or(Ty::Error)))
            }

            HirExpr::StructLit { struct_id, name, fields, span } => {
                let expected = self.env.structs.get(struct_id).cloned();
                let Some(expected) = expected else {
                    return Ty::Error;
                };
                let mut provided: IndexMap<SmolStr, &HirExpr> = IndexMap::new();
                for (n, e) in fields { provided.insert(n.clone(), e); }
                for (fname, fty) in &expected {
                    match provided.get(fname) {
                        Some(e) => {
                            let actual = self.infer_expr(e);
                            self.expect_type(fty, &actual, e.span());
                        }
                        None => self.errors.push(TypeError::MissingField {
                            name: name.to_string(), field: fname.clone(), span: span.clone(),
                        }),
                    }
                }
                for (pname, _) in provided.iter() {
                    if !expected.contains_key(pname) {
                        self.errors.push(TypeError::ExtraField {
                            name: name.to_string(), field: pname.clone(), span: span.clone(),
                        });
                    }
                }
                Ty::Struct(*struct_id)
            }

            HirExpr::Match { val, arms, .. } => {
                let _scrutinee = self.infer_expr(val);
                let mut arm_ty: Option<Ty> = None;
                for arm in arms {
                    let t = self.infer_expr(&arm.body);
                    if let Some(ref expected) = arm_ty {
                        self.expect_type(expected, &t, arm.body.span());
                    } else {
                        arm_ty = Some(t);
                    }
                }
                arm_ty.unwrap_or(Ty::Void)
            }

            HirExpr::Block(b) => {
                self.check_block(b);
                Ty::Void
            }

            HirExpr::Borrow { mutable, val, .. } => {
                let inner = self.infer_expr(val);
                Ty::Ref { mutable: *mutable, inner: Box::new(inner) }
            }

            HirExpr::Assign { target, value, span } => {
                let lhs = self.infer_expr(target);
                let rhs = self.infer_expr(value);
                self.expect_type(&lhs, &rhs, span.clone());
                Ty::Void
            }
        }
    }

    fn infer_call(&mut self, callee: &HirExpr, args: &[HirExpr], span: Span) -> Ty {
        match callee {
            HirExpr::Builtin { name, .. } => {
                let Some((params, ret)) = builtin_signature(name) else {
                    self.errors.push(TypeError::NotCallable {
                        name: name.to_string(), span,
                    });
                    return Ty::Error;
                };
                // Built-ins variádicos (print/println) aceitam exatamente
                // 1 argumento por chamada na v0.1. Para varargs reais,
                // expandir o ABI futuramente.
                self.check_call_args(&params, args, span);
                ret
            }
            HirExpr::Name { id, name, .. } => {
                match self.env.fns.get(id).cloned() {
                    Some(sig) => {
                        self.check_call_args(&sig.params, args, span);
                        sig.ret
                    }
                    None => {
                        self.errors.push(TypeError::NotCallable {
                            name: name.to_string(), span,
                        });
                        Ty::Error
                    }
                }
            }
            _ => {
                self.errors.push(TypeError::NotCallable {
                    name: "(expressão)".into(), span,
                });
                Ty::Error
            }
        }
    }

    fn check_call_args(&mut self, expected: &[Ty], args: &[HirExpr], span: Span) {
        if expected.len() != args.len() {
            self.errors.push(TypeError::BadArity {
                expected: expected.len(),
                found: args.len(),
                span: span.clone(),
            });
        }
        for (i, arg) in args.iter().enumerate() {
            let actual = self.infer_expr(arg);
            if let Some(exp) = expected.get(i) {
                self.expect_type(exp, &actual, arg.span());
            }
        }
    }

    fn check_binop(&mut self, op: BinOp, l: &Ty, r: &Ty, span: Span) -> Ty {
        use BinOp::*;
        if l.is_error() || r.is_error() { return Ty::Error; }

        match op {
            Add | Sub | Mul | Div | Mod => {
                if l == r && l.is_numeric() {
                    return l.clone();
                }
                self.errors.push(TypeError::BadBinOp {
                    op: bin_op_str(op), lhs: l.to_string(), rhs: r.to_string(), span,
                });
                Ty::Error
            }
            Lt | Gt | Lte | Gte => {
                if l == r && l.is_numeric() {
                    return Ty::Bool;
                }
                self.errors.push(TypeError::BadBinOp {
                    op: bin_op_str(op), lhs: l.to_string(), rhs: r.to_string(), span,
                });
                Ty::Error
            }
            Eq | Neq => {
                if l == r {
                    return Ty::Bool;
                }
                self.errors.push(TypeError::BadBinOp {
                    op: bin_op_str(op), lhs: l.to_string(), rhs: r.to_string(), span,
                });
                Ty::Error
            }
            And | Or => {
                if unify(l, &Ty::Bool) && unify(r, &Ty::Bool) {
                    return Ty::Bool;
                }
                self.errors.push(TypeError::BadBinOp {
                    op: bin_op_str(op), lhs: l.to_string(), rhs: r.to_string(), span,
                });
                Ty::Error
            }
        }
    }

    fn const_type(&self, id: DefId) -> Option<Ty> {
        for item in &self.env.module.items {
            if let HirItem::Const(c) = item {
                if c.id == id {
                    return Some(lower_hir_type(&c.ty, self.self_ty.as_ref()));
                }
            }
        }
        None
    }
}

fn bin_op_str(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+", BinOp::Sub => "-", BinOp::Mul => "*",
        BinOp::Div => "/", BinOp::Mod => "%",
        BinOp::Eq => "==", BinOp::Neq => "!=",
        BinOp::Lt => "<", BinOp::Gt => ">",
        BinOp::Lte => "<=", BinOp::Gte => ">=",
        BinOp::And => "&&", BinOp::Or => "||",
    }
}

// ── extensão: helper para pegar span de qualquer HirExpr ───────────────
// (vex-hir não expõe span() — reimplementamos local para evitar churn no crate)
trait HirExprSpan { fn span(&self) -> Span; }
impl HirExprSpan for HirExpr {
    fn span(&self) -> Span {
        match self {
            HirExpr::Int(_, s) | HirExpr::Float(_, s)
            | HirExpr::Str(_, s) | HirExpr::Bool(_, s)
            | HirExpr::SelfRef(s) | HirExpr::Array(_, s) => s.clone(),
            HirExpr::Name { span, .. } | HirExpr::Builtin { span, .. }
            | HirExpr::BinOp { span, .. } | HirExpr::UnaryOp { span, .. }
            | HirExpr::Call { span, .. } | HirExpr::MethodCall { span, .. }
            | HirExpr::FieldAccess { span, .. } | HirExpr::Index { span, .. }
            | HirExpr::StructLit { span, .. } | HirExpr::Match { span, .. }
            | HirExpr::Borrow { span, .. } | HirExpr::Assign { span, .. } => span.clone(),
            HirExpr::Block(b) => b.span.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vex_hir::resolve;
    use vex_parser::parse;

    fn check(src: &str) -> Vec<TypeError> {
        let ast = parse(src).expect("parse ok");
        let (hir, rerrs) = resolve(&ast);
        assert!(rerrs.is_empty(), "resolve errors: {rerrs:?}");
        check_module(&hir)
    }

    #[test]
    fn empty_program_ok() {
        assert!(check("").is_empty());
    }

    #[test]
    fn arithmetic_int_ok() {
        assert!(check("fn t() -> int { return 1 + 2 * 3 }").is_empty());
    }

    #[test]
    fn arithmetic_mixed_int_float_is_error() {
        let errs = check("fn t() -> int { return 1 + 2.5 }");
        assert!(errs.iter().any(|e| matches!(e, TypeError::BadBinOp { .. })));
    }

    #[test]
    fn return_type_mismatch_caught() {
        let errs = check(r#"fn t() -> int { return "x" }"#);
        assert!(!errs.is_empty());
    }

    #[test]
    fn let_type_ann_enforced() {
        let errs = check(r#"fn t() -> void { let x: int = "x" }"#);
        assert!(errs.iter().any(|e| matches!(e, TypeError::Mismatch { .. })));
    }

    #[test]
    fn if_cond_must_be_bool() {
        let errs = check("fn t() -> void { if 1 { } }");
        assert!(errs.iter().any(|e| matches!(e, TypeError::NonBoolCond { .. })));
    }

    #[test]
    fn comparison_returns_bool() {
        assert!(check("fn t() -> bool { return 1 < 2 }").is_empty());
    }

    #[test]
    fn call_arity_mismatch() {
        let src = "fn f(a: int, b: int) -> int { return a + b } fn t() -> int { return f(1) }";
        let errs = check(src);
        assert!(errs.iter().any(|e| matches!(e, TypeError::BadArity { .. })));
    }

    #[test]
    fn call_arg_type_mismatch() {
        let src = r#"fn f(a: int) -> int { return a } fn t() -> int { return f("x") }"#;
        let errs = check(src);
        assert!(errs.iter().any(|e| matches!(e, TypeError::Mismatch { .. })));
    }

    #[test]
    fn struct_field_access_ok() {
        let src = "struct P { x: int } fn t(p: P) -> int { return p.x }";
        assert!(check(src).is_empty());
    }

    #[test]
    fn struct_unknown_field_is_error() {
        let src = "struct P { x: int } fn t(p: P) -> int { return p.y }";
        let errs = check(src);
        assert!(errs.iter().any(|e| matches!(e, TypeError::UnknownField { .. })));
    }

    #[test]
    fn struct_literal_missing_field() {
        let src = "struct P { x: int, y: int } fn t() -> P { return P { x: 1 } }";
        let errs = check(src);
        assert!(errs.iter().any(|e| matches!(e, TypeError::MissingField { .. })));
    }

    #[test]
    fn struct_literal_extra_field() {
        let src = "struct P { x: int } fn t() -> P { return P { x: 1, z: 2 } }";
        let errs = check(src);
        assert!(errs.iter().any(|e| matches!(e, TypeError::ExtraField { .. })));
    }

    #[test]
    fn method_dispatch_ok() {
        let src = "struct P { x: int } \
                   impl P { fn get(self) -> int { return self.x } } \
                   fn t(p: P) -> int { return p.get() }";
        assert!(check(src).is_empty());
    }

    #[test]
    fn unknown_method_is_error() {
        let src = "struct P { x: int } fn t(p: P) -> int { return p.foo() }";
        let errs = check(src);
        assert!(errs.iter().any(|e| matches!(e, TypeError::UnknownMethod { .. })));
    }

    #[test]
    fn index_non_int_is_error() {
        let src = r#"fn t(xs: [int]) -> int { return xs["a"] }"#;
        let errs = check(src);
        assert!(errs.iter().any(|e| matches!(e, TypeError::NonIntIndex { .. })));
    }

    #[test]
    fn index_non_array_is_error() {
        let src = "fn t() -> int { let x = 5 return x[0] }";
        let errs = check(src);
        assert!(errs.iter().any(|e| matches!(e, TypeError::NotIndexable { .. })));
    }

    #[test]
    fn array_homogeneous_required() {
        let src = r#"fn t() -> void { let xs = [1, "two"] }"#;
        let errs = check(src);
        assert!(errs.iter().any(|e| matches!(e, TypeError::Mismatch { .. })));
    }

    #[test]
    fn unary_not_on_bool_ok() {
        assert!(check("fn t() -> bool { return !true }").is_empty());
    }

    #[test]
    fn unary_not_on_int_error() {
        let errs = check("fn t() -> bool { return !5 }");
        assert!(errs.iter().any(|e| matches!(e, TypeError::BadUnaryOp { .. })));
    }

    #[test]
    fn builtin_println_accepts_anything() {
        assert!(check(r#"fn main() -> void { println("hi") println(42) }"#).is_empty());
    }

    #[test]
    fn fib_example_typechecks() {
        let src = include_str!("../../../examples/fib.vex");
        let errs = check(src);
        assert!(errs.is_empty(), "{errs:?}");
    }
}
