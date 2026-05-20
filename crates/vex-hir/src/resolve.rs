//! Name resolution — AST → HIR.
//!
//! Estratégia em duas passagens:
//!   1. **Collect:** registra todos os items top-level (fn, struct, const)
//!      em uma tabela de defs. Permite forward references (uma fn pode
//!      chamar outra declarada mais abaixo).
//!   2. **Resolve:** percorre corpos resolvendo cada identificador em um
//!      `DefId`. Escopos lexicais empilhados para variáveis locais.
//!
//! Erros são acumulados e reportados; o resolver tenta continuar para
//! reportar tantos erros quanto possível em uma única passagem.

use indexmap::IndexMap;
use smol_str::SmolStr;
use vex_ast as ast;

use crate::hir::*;

pub type Span = std::ops::Range<usize>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum ResolveError {
    #[error("nome `{name}` não declarado neste escopo")]
    Unknown { name: SmolStr, span: Span },

    #[error("nome `{name}` declarado duas vezes no mesmo módulo")]
    Duplicate { name: SmolStr, span: Span, first: Span },

    #[error("tipo `{name}` não declarado")]
    UnknownType { name: SmolStr, span: Span },

    #[error("struct `{name}` não existe — não é possível instanciar literal")]
    UnknownStruct { name: SmolStr, span: Span },

    #[error("`self` só pode ser usado dentro de métodos (impl blocks)")]
    SelfOutsideMethod { span: Span },

    #[error("impl block para tipo `{target}` que não é struct conhecido")]
    ImplOnUnknownType { target: SmolStr, span: Span },

    #[error("atribuição inválida: alvo não é uma lvalue")]
    InvalidAssignTarget { span: Span },
}

impl ResolveError {
    pub fn span(&self) -> &Span {
        match self {
            ResolveError::Unknown { span, .. }
            | ResolveError::Duplicate { span, .. }
            | ResolveError::UnknownType { span, .. }
            | ResolveError::UnknownStruct { span, .. }
            | ResolveError::SelfOutsideMethod { span }
            | ResolveError::ImplOnUnknownType { span, .. }
            | ResolveError::InvalidAssignTarget { span } => span,
        }
    }
}

/// Resolve o módulo AST. Retorna o HIR + lista de erros.
///
/// Se a lista de erros estiver vazia, o HIR é válido para a próxima fase.
/// Caso contrário, o HIR pode conter `Unresolved`/`DefId::INVALID` em pontos
/// que falharam — não deve ser passado adiante.
pub fn resolve(module: &ast::Module) -> (HirModule, Vec<ResolveError>) {
    let mut r = Resolver::new();
    r.collect_items(module);
    r.resolve_items(module);
    (
        HirModule { defs: r.defs, items: r.items },
        r.errors,
    )
}

// ── Implementação ───────────────────────────────────────────────────────

struct Resolver {
    defs: Vec<Def>,
    /// Top-level: nome → DefId. Inclui fns, structs, consts.
    globals: IndexMap<SmolStr, DefId>,
    /// Pilha de escopos lexicais para locals/params.
    scopes: Vec<IndexMap<SmolStr, DefId>>,
    /// `Some(struct_id)` enquanto estamos dentro de um impl block.
    current_impl: Option<DefId>,
    items: Vec<HirItem>,
    errors: Vec<ResolveError>,
}

impl Resolver {
    fn new() -> Self {
        Self {
            defs: Vec::new(),
            globals: IndexMap::new(),
            scopes: Vec::new(),
            current_impl: None,
            items: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn alloc_def(&mut self, name: SmolStr, kind: DefKind, span: Span) -> DefId {
        let id = DefId(self.defs.len() as u32);
        self.defs.push(Def { name, kind, span });
        id
    }

    fn declare_global(&mut self, name: SmolStr, kind: DefKind, span: Span) -> DefId {
        if let Some(existing) = self.globals.get(&name) {
            let first = self.defs[existing.0 as usize].span.clone();
            self.errors.push(ResolveError::Duplicate {
                name: name.clone(), span: span.clone(), first,
            });
        }
        let id = self.alloc_def(name.clone(), kind, span);
        self.globals.insert(name, id);
        id
    }

    fn enter_scope(&mut self) { self.scopes.push(IndexMap::new()); }
    fn exit_scope(&mut self) { self.scopes.pop(); }

    fn declare_local(&mut self, name: SmolStr, kind: DefKind, span: Span) -> DefId {
        let id = self.alloc_def(name.clone(), kind, span);
        // Shadowing permitido: simplesmente sobrescreve no escopo atual.
        if let Some(top) = self.scopes.last_mut() {
            top.insert(name, id);
        }
        id
    }

    /// Procura `name` em escopos locais (do mais interno ao externo),
    /// depois nos globais. `None` se não encontrado.
    fn lookup(&self, name: &SmolStr) -> Option<DefId> {
        for scope in self.scopes.iter().rev() {
            if let Some(id) = scope.get(name) { return Some(*id); }
        }
        self.globals.get(name).copied()
    }

    // ── Passagem 1: collect top-level items ──────────────────────────────

    fn collect_items(&mut self, module: &ast::Module) {
        for item in &module.items {
            match item {
                ast::Item::Fn(f) => {
                    let _ = self.declare_global(f.name.clone(), DefKind::Fn, f.span.clone());
                }
                ast::Item::Struct(s) => {
                    let _ = self.declare_global(s.name.clone(), DefKind::Struct, s.span.clone());
                }
                ast::Item::Const(c) => {
                    let _ = self.declare_global(c.name.clone(), DefKind::Const, c.span.clone());
                }
                ast::Item::Impl(_) | ast::Item::Use(_) => {
                    // Impls não introduzem nome top-level. Use ignorado por
                    // enquanto (sem stdlib formal — Fase 8).
                }
            }
        }
    }

    // ── Passagem 2: resolve bodies ──────────────────────────────────────

    fn resolve_items(&mut self, module: &ast::Module) {
        for item in &module.items {
            match item {
                ast::Item::Fn(f)     => { let hf = self.resolve_fn(f); self.items.push(HirItem::Fn(hf)); }
                ast::Item::Struct(s) => { let hs = self.resolve_struct(s); self.items.push(HirItem::Struct(hs)); }
                ast::Item::Const(c)  => { let hc = self.resolve_const(c); self.items.push(HirItem::Const(hc)); }
                ast::Item::Impl(i)   => { let hi = self.resolve_impl(i); self.items.push(HirItem::Impl(hi)); }
                ast::Item::Use(_)    => { /* ignorado por ora */ }
            }
        }
    }

    fn resolve_struct(&mut self, s: &ast::StructDecl) -> HirStruct {
        let id = *self.globals.get(&s.name).expect("collected in pass 1");
        let mut fields = IndexMap::new();
        for (fname, fty, fspan) in &s.fields {
            let ty = self.resolve_type(fty, fspan.clone());
            fields.insert(fname.clone(), HirField {
                name: fname.clone(), ty, span: fspan.clone(),
            });
        }
        HirStruct { id, name: s.name.clone(), fields, is_pub: s.is_pub, span: s.span.clone() }
    }

    fn resolve_const(&mut self, c: &ast::ConstDecl) -> HirConst {
        let id = *self.globals.get(&c.name).expect("collected in pass 1");
        let ty = self.resolve_type(&c.ty, c.span.clone());
        // const é avaliado num "escopo vazio" — só consts/fns visíveis.
        self.enter_scope();
        let value = self.resolve_expr(&c.value);
        self.exit_scope();
        HirConst { id, name: c.name.clone(), ty, value, is_pub: c.is_pub, span: c.span.clone() }
    }

    fn resolve_impl(&mut self, i: &ast::ImplBlock) -> HirImpl {
        let target = match self.globals.get(&i.target) {
            Some(id) if self.defs[id.0 as usize].kind == DefKind::Struct => *id,
            _ => {
                self.errors.push(ResolveError::ImplOnUnknownType {
                    target: i.target.clone(), span: i.span.clone(),
                });
                // continuar com placeholder para não abortar
                self.alloc_def(i.target.clone(), DefKind::Struct, i.span.clone())
            }
        };

        let prev = self.current_impl.replace(target);
        let methods: Vec<HirFn> = i.methods.iter().map(|m| self.resolve_fn(m)).collect();
        self.current_impl = prev;

        HirImpl { target, methods, span: i.span.clone() }
    }

    fn resolve_fn(&mut self, f: &ast::FnDecl) -> HirFn {
        // Para métodos dentro de impl, a fn não está em globals; criamos
        // um DefId local para ela.
        let id = if self.current_impl.is_some() {
            self.alloc_def(f.name.clone(), DefKind::Fn, f.span.clone())
        } else {
            *self.globals.get(&f.name).expect("collected in pass 1")
        };

        self.enter_scope();
        let mut params = Vec::with_capacity(f.params.len());
        for p in &f.params {
            let kind = if p.name == "self" { DefKind::SelfParam } else { DefKind::Param };
            let ty = self.resolve_type(&p.ty, p.span.clone());
            let pid = self.declare_local(p.name.clone(), kind, p.span.clone());
            params.push(HirParam {
                id: pid, name: p.name.clone(), ty, mutable: p.mutable, span: p.span.clone(),
            });
        }
        let ret_type = self.resolve_type(&f.ret_type, f.span.clone());
        let body = self.resolve_block(&f.body);
        self.exit_scope();

        HirFn {
            id, name: f.name.clone(), params, ret_type, body,
            is_pub: f.is_pub, is_comptime: f.is_comptime, span: f.span.clone(),
        }
    }

    fn resolve_type(&mut self, t: &ast::Type, ctx_span: Span) -> HirType {
        match t {
            ast::Type::Int   => HirType::Int,
            ast::Type::Float => HirType::Float,
            ast::Type::Bool  => HirType::Bool,
            ast::Type::Str   => HirType::Str,
            ast::Type::Void  => HirType::Void,
            ast::Type::Infer => HirType::Unresolved("_".into()),
            ast::Type::Named(name) => {
                if name == "Self" {
                    if self.current_impl.is_none() {
                        self.errors.push(ResolveError::SelfOutsideMethod { span: ctx_span });
                    }
                    return HirType::SelfTy;
                }
                match self.globals.get(name) {
                    Some(id) if self.defs[id.0 as usize].kind == DefKind::Struct => HirType::Struct(*id),
                    _ => {
                        self.errors.push(ResolveError::UnknownType {
                            name: name.clone(), span: ctx_span,
                        });
                        HirType::Unresolved(name.clone())
                    }
                }
            }
            ast::Type::Array(inner) => HirType::Array(Box::new(self.resolve_type(inner, ctx_span))),
            ast::Type::Ref { mutable, inner } => HirType::Ref {
                mutable: *mutable,
                inner: Box::new(self.resolve_type(inner, ctx_span)),
            },
            ast::Type::Fn(_, _) => HirType::Unresolved("fn".into()), // fn types pós-MVP
        }
    }

    fn resolve_block(&mut self, b: &ast::Block) -> HirBlock {
        self.enter_scope();
        let stmts = b.stmts.iter().map(|s| self.resolve_stmt(s)).collect();
        self.exit_scope();
        HirBlock { stmts, span: b.span.clone() }
    }

    fn resolve_stmt(&mut self, s: &ast::Stmt) -> HirStmt {
        match s {
            ast::Stmt::Let { name, mutable, type_ann, value, span } => {
                let value = self.resolve_expr(value);
                let type_ann = type_ann.as_ref().map(|t| self.resolve_type(t, span.clone()));
                let id = self.declare_local(name.clone(), DefKind::Local, span.clone());
                HirStmt::Let {
                    id, name: name.clone(), mutable: *mutable,
                    type_ann, value, span: span.clone(),
                }
            }
            ast::Stmt::Return(e, span) => HirStmt::Return(
                e.as_ref().map(|e| self.resolve_expr(e)),
                span.clone(),
            ),
            ast::Stmt::If { cond, then_body, else_body, span } => HirStmt::If {
                cond: self.resolve_expr(cond),
                then_body: self.resolve_block(then_body),
                else_body: else_body.as_ref().map(|b| self.resolve_block(b)),
                span: span.clone(),
            },
            ast::Stmt::While { cond, body, span } => HirStmt::While {
                cond: self.resolve_expr(cond),
                body: self.resolve_block(body),
                span: span.clone(),
            },
            ast::Stmt::For { var, iter, body, span } => {
                let iter = self.resolve_expr(iter);
                self.enter_scope();
                let var_id = self.declare_local(var.clone(), DefKind::Local, span.clone());
                let body = self.resolve_block(body);
                self.exit_scope();
                HirStmt::For {
                    var_id, var_name: var.clone(), iter, body, span: span.clone(),
                }
            }
            ast::Stmt::Break(s) => HirStmt::Break(s.clone()),
            ast::Stmt::Continue(s) => HirStmt::Continue(s.clone()),
            ast::Stmt::Expr(e) => HirStmt::Expr(self.resolve_expr(e)),
        }
    }

    fn resolve_expr(&mut self, e: &ast::Expr) -> HirExpr {
        use ast::Expr::*;
        match e {
            Int(v, s)   => HirExpr::Int(*v, s.clone()),
            Float(v, s) => HirExpr::Float(*v, s.clone()),
            Str(v, s)   => HirExpr::Str(v.clone(), s.clone()),
            Bool(v, s)  => HirExpr::Bool(*v, s.clone()),
            SelfRef(s)  => {
                if self.current_impl.is_none() {
                    self.errors.push(ResolveError::SelfOutsideMethod { span: s.clone() });
                }
                HirExpr::SelfRef(s.clone())
            }
            Ident(name, s) => {
                match self.lookup(name) {
                    Some(id) => HirExpr::Name { id, name: name.clone(), span: s.clone() },
                    None => {
                        if is_builtin(name) {
                            HirExpr::Builtin { name: name.clone(), span: s.clone() }
                        } else {
                            self.errors.push(ResolveError::Unknown {
                                name: name.clone(), span: s.clone(),
                            });
                            // Continua com builtin placeholder para não cascatear.
                            HirExpr::Builtin { name: name.clone(), span: s.clone() }
                        }
                    }
                }
            }
            BinOp { op, left, right, span } => HirExpr::BinOp {
                op: *op,
                left: Box::new(self.resolve_expr(left)),
                right: Box::new(self.resolve_expr(right)),
                span: span.clone(),
            },
            UnaryOp { op, val, span } => HirExpr::UnaryOp {
                op: *op,
                val: Box::new(self.resolve_expr(val)),
                span: span.clone(),
            },
            Call { callee, args, span } => HirExpr::Call {
                callee: Box::new(self.resolve_expr(callee)),
                args: args.iter().map(|a| self.resolve_expr(a)).collect(),
                span: span.clone(),
            },
            MethodCall { receiver, name, args, span } => HirExpr::MethodCall {
                receiver: Box::new(self.resolve_expr(receiver)),
                name: name.clone(),
                args: args.iter().map(|a| self.resolve_expr(a)).collect(),
                span: span.clone(),
            },
            FieldAccess { obj, field, span } => HirExpr::FieldAccess {
                obj: Box::new(self.resolve_expr(obj)),
                field: field.clone(),
                span: span.clone(),
            },
            Index { obj, idx, span } => HirExpr::Index {
                obj: Box::new(self.resolve_expr(obj)),
                idx: Box::new(self.resolve_expr(idx)),
                span: span.clone(),
            },
            Array(items, span) => HirExpr::Array(
                items.iter().map(|i| self.resolve_expr(i)).collect(),
                span.clone(),
            ),
            StructLit { name, fields, span } => {
                let struct_id = match self.globals.get(name) {
                    Some(id) if self.defs[id.0 as usize].kind == DefKind::Struct => *id,
                    _ => {
                        self.errors.push(ResolveError::UnknownStruct {
                            name: name.clone(), span: span.clone(),
                        });
                        self.alloc_def(name.clone(), DefKind::Struct, span.clone())
                    }
                };
                let fields = fields.iter()
                    .map(|(n, e)| (n.clone(), self.resolve_expr(e)))
                    .collect();
                HirExpr::StructLit {
                    struct_id, name: name.clone(), fields, span: span.clone(),
                }
            }
            Match { val, arms, span } => {
                let val = Box::new(self.resolve_expr(val));
                let arms = arms.iter().map(|a| {
                    self.enter_scope();
                    let pattern = self.resolve_pattern(&a.pattern);
                    let body = self.resolve_expr(&a.body);
                    self.exit_scope();
                    HirMatchArm { pattern, body, span: a.span.clone() }
                }).collect();
                HirExpr::Match { val, arms, span: span.clone() }
            }
            Block(b) => HirExpr::Block(self.resolve_block(b)),
            Ref { mutable, val, span } => HirExpr::Borrow {
                mutable: *mutable,
                val: Box::new(self.resolve_expr(val)),
                span: span.clone(),
            },
            Assign { target, value, span } => {
                let target_hir = self.resolve_expr(target);
                if !is_lvalue(&target_hir) {
                    self.errors.push(ResolveError::InvalidAssignTarget {
                        span: span.clone(),
                    });
                }
                HirExpr::Assign {
                    target: Box::new(target_hir),
                    value: Box::new(self.resolve_expr(value)),
                    span: span.clone(),
                }
            }
        }
    }

    fn resolve_pattern(&mut self, p: &ast::Pattern) -> HirPattern {
        match p {
            ast::Pattern::Int(v)   => HirPattern::Int(*v),
            ast::Pattern::Bool(v)  => HirPattern::Bool(*v),
            ast::Pattern::Str(v)   => HirPattern::Str(v.clone()),
            ast::Pattern::Wildcard => HirPattern::Wildcard,
            ast::Pattern::Range { lo, hi, inclusive } => HirPattern::Range {
                lo: *lo, hi: *hi, inclusive: *inclusive,
            },
            ast::Pattern::Ident(n) => {
                // Em match arms, identifier binda variável local — sempre.
                // Equivalente ao Rust: `x` em match captura.
                let id = self.declare_local(n.clone(), DefKind::Local, 0..0);
                HirPattern::Binding { id, name: n.clone() }
            }
        }
    }
}

fn is_lvalue(e: &HirExpr) -> bool {
    matches!(e,
        HirExpr::Name { .. }
        | HirExpr::FieldAccess { .. }
        | HirExpr::Index { .. }
        | HirExpr::SelfRef(_))
}

/// Built-ins reconhecidos enquanto a stdlib formal (Fase 8) não existe.
/// Lista pequena, intencional. typeck dá erro para chamadas com tipos
/// errados; resolver só evita marcar como "Unknown".
fn is_builtin(name: &SmolStr) -> bool {
    matches!(name.as_str(),
        "print" | "println" | "input"
        | "read_file" | "write_file"
        | "to_int" | "to_float" | "to_str"
        | "len" | "push" | "pop"
        | "sqrt" | "abs" | "min" | "max"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vex_parser::parse;

    fn resolve_str(src: &str) -> (HirModule, Vec<ResolveError>) {
        let m = parse(src).expect("parse ok");
        resolve(&m)
    }

    #[test]
    fn resolve_empty() {
        let (h, errs) = resolve_str("");
        assert!(errs.is_empty());
        assert!(h.items.is_empty());
        assert!(h.defs.is_empty());
    }

    #[test]
    fn resolve_simple_fn() {
        let (h, errs) = resolve_str("fn main() -> void { }");
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(h.items.len(), 1);
        assert_eq!(h.defs.len(), 1);
        assert_eq!(h.defs[0].kind, DefKind::Fn);
    }

    #[test]
    fn resolve_fib_example() {
        let src = include_str!("../../../examples/fib.vex");
        let (_, errs) = resolve_str(src);
        assert!(errs.is_empty(), "{errs:?}");
    }

    #[test]
    fn resolve_ponto_example() {
        let src = include_str!("../../../examples/ponto.vex");
        let (_, errs) = resolve_str(src);
        assert!(errs.is_empty(), "{errs:?}");
    }

    #[test]
    fn unknown_var_is_error() {
        let (_, errs) = resolve_str("fn main() -> void { let x = y }");
        assert_eq!(errs.len(), 1);
        assert!(matches!(errs[0], ResolveError::Unknown { .. }));
    }

    #[test]
    fn duplicate_top_level_is_error() {
        let (_, errs) = resolve_str("fn a() -> void { } fn a() -> void { }");
        assert_eq!(errs.len(), 1);
        assert!(matches!(errs[0], ResolveError::Duplicate { .. }));
    }

    #[test]
    fn unknown_type_is_error() {
        let (_, errs) = resolve_str("fn t(x: Foo) -> void { }");
        assert!(errs.iter().any(|e| matches!(e, ResolveError::UnknownType { .. })));
    }

    #[test]
    fn struct_literal_unknown_is_error() {
        let (_, errs) = resolve_str("fn t() -> void { let p = Bar { x: 1 } }");
        assert!(errs.iter().any(|e| matches!(e, ResolveError::UnknownStruct { .. })));
    }

    #[test]
    fn self_outside_method_is_error() {
        let (_, errs) = resolve_str("fn t() -> void { let x = self }");
        assert!(errs.iter().any(|e| matches!(e, ResolveError::SelfOutsideMethod { .. })));
    }

    #[test]
    fn forward_reference_allowed() {
        // call b() em a() antes de b ser declarada → válido (coletadas em pass 1)
        let src = "fn a() -> void { b() } fn b() -> void { }";
        let (_, errs) = resolve_str(src);
        assert!(errs.is_empty(), "{errs:?}");
    }

    #[test]
    fn builtin_print_is_recognized() {
        let src = r#"fn main() -> void { println("hi") }"#;
        let (_, errs) = resolve_str(src);
        assert!(errs.is_empty(), "{errs:?}");
    }

    #[test]
    fn shadowing_in_inner_scope() {
        // `let x = 1; { let x = 2; ... }` → ambos OK, sem erro
        let src = "fn t() -> void { let x = 1 if true { let x = 2 } }";
        let (_, errs) = resolve_str(src);
        assert!(errs.is_empty(), "{errs:?}");
    }

    #[test]
    fn invalid_assign_target_is_error() {
        let src = "fn t() -> void { let x = 0  5 = x }";
        let (_, errs) = resolve_str(src);
        assert!(errs.iter().any(|e| matches!(e, ResolveError::InvalidAssignTarget { .. })));
    }

    #[test]
    fn impl_block_resolves_methods() {
        let src = r#"
            struct P { x: int }
            impl P {
                fn get(self) -> int { return self.x }
            }
        "#;
        let (h, errs) = resolve_str(src);
        assert!(errs.is_empty(), "{errs:?}");
        // 2 defs globais (P, P::get?) — get é alocada localmente
        assert!(h.defs.iter().any(|d| d.name == "P" && d.kind == DefKind::Struct));
    }
}
