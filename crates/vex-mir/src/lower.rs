//! Lowering HIR → MIR.
//!
//! Algoritmo recursivo:
//! - cada expressão complexa é desmontada em assignments para temporários
//! - control flow (`if`, `while`, `for`) cria blocos novos + terminators
//! - blocos são fechados com terminator antes de continuar
//!
//! Premissa: o HIR já passou pelo typeck. Assumimos que tipos são
//! consistentes — não revalidamos.

use indexmap::IndexMap;
use smol_str::SmolStr;
use vex_ast::{BinOp, UnaryOp};
use vex_hir::{
    DefId, HirBlock, HirExpr, HirFn, HirItem, HirModule, HirStmt,
};
use vex_typeck::Ty;

/// Tabela auxiliar para o lowerer consultar tipos de campos sem
/// reimplementar typeck.
type StructFieldsTable = IndexMap<DefId, IndexMap<SmolStr, Ty>>;

use crate::mir::*;

#[derive(Debug, thiserror::Error)]
pub enum LowerError {
    #[error("lowering falhou: {0}")]
    Generic(String),
}

/// Lowering top-level. Constrói `MirModule` com fns + structs.
///
/// Assume que typeck já validou o módulo — não recomputa tipos.
pub fn lower_module(
    hir: &HirModule,
    type_of_def: &dyn Fn(DefId) -> Option<Ty>,
) -> Result<MirModule, LowerError> {
    let structs: Vec<MirStruct> = hir.items.iter().filter_map(|i| match i {
        HirItem::Struct(s) => Some(MirStruct {
            id: s.id,
            name: s.name.clone(),
            fields: s.fields.iter()
                .map(|(n, f)| (n.clone(), lower_ty(&f.ty, None)))
                .collect(),
        }),
        _ => None,
    }).collect();

    let mut struct_fields: StructFieldsTable = IndexMap::new();
    for s in &structs {
        let map: IndexMap<SmolStr, Ty> = s.fields.iter().cloned().collect();
        struct_fields.insert(s.id, map);
    }

    // Tabela `(struct_id, name) → ret_ty` para inferir tipo de retorno
    // de method calls dentro do lowerer.
    let mut method_ret: IndexMap<(DefId, SmolStr), Ty> = IndexMap::new();
    for item in &hir.items {
        if let HirItem::Impl(im) = item {
            for m in &im.methods {
                method_ret.insert(
                    (im.target, m.name.clone()),
                    lower_ty(&m.ret_type, Some(&Ty::Struct(im.target))),
                );
            }
        }
    }

    let mut fns = Vec::new();
    let mut methods = Vec::new();
    for item in &hir.items {
        match item {
            HirItem::Fn(f) => fns.push(lower_fn(f, None, type_of_def, &struct_fields, &method_ret)?),
            HirItem::Impl(im) => {
                let self_ty = Ty::Struct(im.target);
                for m in &im.methods {
                    methods.push(MirMethod {
                        struct_id: im.target,
                        name: m.name.clone(),
                        fn_id: m.id,
                    });
                    fns.push(lower_fn(m, Some(&self_ty), type_of_def, &struct_fields, &method_ret)?);
                }
            }
            _ => {}
        }
    }

    Ok(MirModule { fns, structs, methods })
}

fn lower_ty(t: &vex_hir::HirType, self_ty: Option<&Ty>) -> Ty {
    vex_typeck::lower_hir_type(t, self_ty)
}

fn lower_fn(
    f: &HirFn,
    self_ty: Option<&Ty>,
    type_of_def: &dyn Fn(DefId) -> Option<Ty>,
    struct_fields: &StructFieldsTable,
    method_ret: &IndexMap<(DefId, SmolStr), Ty>,
) -> Result<MirFn, LowerError> {
    let ret_ty = lower_ty(&f.ret_type, self_ty);
    let mut lowerer = FnLowerer::new(
        self_ty.cloned(), ret_ty.clone(), type_of_def, struct_fields, method_ret,
    );

    // Parâmetros viram primeiros locals.
    let mut params = Vec::new();
    for p in &f.params {
        let ty = if p.name == "self" {
            self_ty.cloned().unwrap_or(Ty::Error)
        } else {
            lower_ty(&p.ty, self_ty)
        };
        let id = lowerer.new_local(p.name.clone(), ty, p.mutable);
        lowerer.def_to_local.insert(p.id, id);
        params.push(id);
    }

    let entry = lowerer.new_block();
    lowerer.current = entry;

    lowerer.lower_block(&f.body);

    // Bloco corrente pode terminar sem terminator explícito (fn void).
    lowerer.finish_open_block_as_return();

    Ok(MirFn {
        id: f.id,
        name: f.name.clone(),
        params,
        locals: lowerer.locals,
        blocks: lowerer.blocks,
        entry,
        ret_ty: lower_ty(&f.ret_type, self_ty),
        span: f.span.clone(),
    })
}

struct FnLowerer<'a> {
    locals: Vec<MirLocal>,
    blocks: Vec<BasicBlock>,
    current: BlockId,
    def_to_local: IndexMap<DefId, LocalId>,
    next_tmp: u32,
    self_ty: Option<Ty>,
    ret_ty: Ty,
    type_of_def: &'a dyn Fn(DefId) -> Option<Ty>,
    struct_fields: &'a StructFieldsTable,
    method_ret: &'a IndexMap<(DefId, SmolStr), Ty>,
    /// Pilha de (continue_target, break_target) para loops.
    loop_targets: Vec<(BlockId, BlockId)>,
}

impl<'a> FnLowerer<'a> {
    fn new(
        self_ty: Option<Ty>,
        ret_ty: Ty,
        type_of_def: &'a dyn Fn(DefId) -> Option<Ty>,
        struct_fields: &'a StructFieldsTable,
        method_ret: &'a IndexMap<(DefId, SmolStr), Ty>,
    ) -> Self {
        Self {
            locals: Vec::new(),
            blocks: Vec::new(),
            current: BlockId(0),
            def_to_local: IndexMap::new(),
            next_tmp: 0,
            self_ty,
            ret_ty,
            type_of_def,
            struct_fields,
            method_ret,
            loop_targets: Vec::new(),
        }
    }

    fn new_local(&mut self, name: SmolStr, ty: Ty, mutable: bool) -> LocalId {
        let id = LocalId(self.locals.len() as u32);
        self.locals.push(MirLocal { id, ty, name, mutable });
        id
    }

    fn new_tmp(&mut self, ty: Ty) -> LocalId {
        let n = self.next_tmp;
        self.next_tmp += 1;
        self.new_local(format!("_t{n}").into(), ty, false)
    }

    fn new_block(&mut self) -> BlockId {
        let id = BlockId(self.blocks.len() as u32);
        self.blocks.push(BasicBlock {
            id,
            stmts: Vec::new(),
            terminator: Terminator::Unreachable,
        });
        id
    }

    fn push_stmt(&mut self, s: Statement) {
        let idx = self.current.0 as usize;
        self.blocks[idx].stmts.push(s);
    }

    fn set_terminator(&mut self, t: Terminator) {
        let idx = self.current.0 as usize;
        self.blocks[idx].terminator = t;
    }

    fn finish_open_block_as_return(&mut self) {
        // Para fn void: fluxo que caiu no fim implica `return`. Para fn
        // com retorno, deixar como `Unreachable` é correto (typeck garante
        // que todos os caminhos válidos têm `return` explícito).
        let idx = self.current.0 as usize;
        if matches!(self.blocks[idx].terminator, Terminator::Unreachable)
            && matches!(self.ret_ty, Ty::Void)
        {
            self.blocks[idx].terminator = Terminator::Return(None);
        }
    }

    // ── Lowering de blocos / statements ─────────────────────────────

    fn lower_block(&mut self, b: &HirBlock) {
        for s in &b.stmts {
            self.lower_stmt(s);
        }
    }

    fn lower_stmt(&mut self, s: &HirStmt) {
        match s {
            HirStmt::Let { id, name, mutable, value, span, .. } => {
                let ty = self.infer_expr_type(value);
                let local = self.new_local(name.clone(), ty, *mutable);
                self.def_to_local.insert(*id, local);
                let rvalue = self.lower_expr_into(value);
                self.push_stmt(Statement::Assign {
                    local, rvalue, span: span.clone(),
                });
            }
            HirStmt::Return(opt, span) => {
                let local = opt.as_ref().map(|e| self.lower_expr_to_local(e));
                self.set_terminator(Terminator::Return(local));
                // Abre bloco morto para statements subsequentes (provavelmente
                // não haverá — return é sempre o último). Mas garante invariantes.
                let dead = self.new_block();
                self.current = dead;
                let _ = span;
            }
            HirStmt::If { cond, then_body, else_body, .. } => {
                let cond_local = self.lower_expr_to_local(cond);
                let then_blk = self.new_block();
                let else_blk = if else_body.is_some() { Some(self.new_block()) } else { None };
                let join = self.new_block();

                self.set_terminator(Terminator::If {
                    cond: cond_local,
                    then: then_blk,
                    otherwise: else_blk.unwrap_or(join),
                });

                // Then.
                self.current = then_blk;
                self.lower_block(then_body);
                self.goto_if_open(join);

                // Else.
                if let Some(eb) = else_blk {
                    self.current = eb;
                    if let Some(else_body) = else_body {
                        self.lower_block(else_body);
                    }
                    self.goto_if_open(join);
                }

                self.current = join;
            }
            HirStmt::While { cond, body, .. } => {
                let head = self.new_block();
                let body_blk = self.new_block();
                let exit = self.new_block();

                // Entrada → head.
                self.set_terminator(Terminator::Goto(head));

                // head: avalia cond, branch.
                self.current = head;
                let cond_local = self.lower_expr_to_local(cond);
                self.set_terminator(Terminator::If {
                    cond: cond_local, then: body_blk, otherwise: exit,
                });

                // body.
                self.current = body_blk;
                self.loop_targets.push((head, exit));
                self.lower_block(body);
                self.loop_targets.pop();
                self.goto_if_open(head);

                self.current = exit;
            }
            HirStmt::For { var_id, var_name, iter, body, .. } => {
                // Açúcar conservador: i = 0; len = len(iter); while i < len { x = iter[i]; body; i = i+1 }
                let iter_ty = self.infer_expr_type(iter);
                let iter_local = self.lower_expr_to_local(iter);

                let elem_ty = match &iter_ty {
                    Ty::Array(inner) => (**inner).clone(),
                    _ => Ty::Error,
                };

                let i = self.new_local("_i".into(), Ty::Int, true);
                self.push_stmt(Statement::Assign {
                    local: i,
                    rvalue: Rvalue::Use(Operand::Const(Const::Int(0))),
                    span: 0..0,
                });
                let len = self.new_local("_len".into(), Ty::Int, false);
                self.push_stmt(Statement::Assign {
                    local: len,
                    rvalue: Rvalue::Call {
                        callee: Callee::Builtin("len".into()),
                        args: vec![Operand::Local(iter_local)],
                    },
                    span: 0..0,
                });

                let head = self.new_block();
                let body_blk = self.new_block();
                let step = self.new_block();
                let exit = self.new_block();

                self.set_terminator(Terminator::Goto(head));

                // head: cond = i < len; branch
                self.current = head;
                let cond_tmp = self.new_tmp(Ty::Bool);
                self.push_stmt(Statement::Assign {
                    local: cond_tmp,
                    rvalue: Rvalue::BinaryOp {
                        op: BinOp::Lt,
                        lhs: Operand::Local(i),
                        rhs: Operand::Local(len),
                    },
                    span: 0..0,
                });
                self.set_terminator(Terminator::If {
                    cond: cond_tmp, then: body_blk, otherwise: exit,
                });

                // body: var = iter[i]; lower body; goto step
                self.current = body_blk;
                let var_local = self.new_local(var_name.clone(), elem_ty, false);
                self.def_to_local.insert(*var_id, var_local);
                self.push_stmt(Statement::Assign {
                    local: var_local,
                    rvalue: Rvalue::Index { obj: iter_local, idx: i },
                    span: 0..0,
                });
                self.loop_targets.push((step, exit));
                self.lower_block(body);
                self.loop_targets.pop();
                self.goto_if_open(step);

                // step: i = i + 1; goto head
                self.current = step;
                let one = self.new_tmp(Ty::Int);
                self.push_stmt(Statement::Assign {
                    local: one,
                    rvalue: Rvalue::Use(Operand::Const(Const::Int(1))),
                    span: 0..0,
                });
                let next_i = self.new_tmp(Ty::Int);
                self.push_stmt(Statement::Assign {
                    local: next_i,
                    rvalue: Rvalue::BinaryOp {
                        op: BinOp::Add,
                        lhs: Operand::Local(i),
                        rhs: Operand::Local(one),
                    },
                    span: 0..0,
                });
                self.push_stmt(Statement::Assign {
                    local: i,
                    rvalue: Rvalue::Use(Operand::Local(next_i)),
                    span: 0..0,
                });
                self.set_terminator(Terminator::Goto(head));

                self.current = exit;
            }
            HirStmt::Break(_) => {
                if let Some(&(_, exit)) = self.loop_targets.last() {
                    self.set_terminator(Terminator::Goto(exit));
                    let dead = self.new_block();
                    self.current = dead;
                }
            }
            HirStmt::Continue(_) => {
                if let Some(&(cont, _)) = self.loop_targets.last() {
                    self.set_terminator(Terminator::Goto(cont));
                    let dead = self.new_block();
                    self.current = dead;
                }
            }
            HirStmt::Expr(e) => {
                let _ = self.lower_expr_to_local(e);
            }
        }
    }

    /// Garante que o bloco atual tenha terminator. Se ainda for Unreachable,
    /// converte em Goto(target).
    fn goto_if_open(&mut self, target: BlockId) {
        let idx = self.current.0 as usize;
        if matches!(self.blocks[idx].terminator, Terminator::Unreachable) {
            self.blocks[idx].terminator = Terminator::Goto(target);
        }
    }

    // ── Lowering de expressões ──────────────────────────────────────

    /// Lowera `e` produzindo um Rvalue. Quando precisar de operandos
    /// não-atômicos, primeiro materializa em temporários e devolve `Use`.
    fn lower_expr_into(&mut self, e: &HirExpr) -> Rvalue {
        match e {
            HirExpr::Int(v, _)   => Rvalue::Use(Operand::Const(Const::Int(*v))),
            HirExpr::Float(v, _) => Rvalue::Use(Operand::Const(Const::Float(*v))),
            HirExpr::Bool(v, _)  => Rvalue::Use(Operand::Const(Const::Bool(*v))),
            HirExpr::Str(v, _)   => Rvalue::Use(Operand::Const(Const::Str(v.clone()))),
            HirExpr::Name { id, .. } => {
                let l = self.def_to_local.get(id).copied();
                if let Some(l) = l {
                    Rvalue::Use(Operand::Local(l))
                } else {
                    // Referência a fn/const: trataremos como builtin placeholder
                    // por enquanto. Codegen real (Fase 6) lê via tabela global.
                    Rvalue::Use(Operand::Const(Const::Unit))
                }
            }
            HirExpr::Builtin { .. } | HirExpr::SelfRef(_) => {
                Rvalue::Use(Operand::Const(Const::Unit))
            }
            HirExpr::BinOp { op, left, right, .. } => {
                let l = self.lower_expr_to_operand(left);
                let r = self.lower_expr_to_operand(right);
                Rvalue::BinaryOp { op: *op, lhs: l, rhs: r }
            }
            HirExpr::UnaryOp { op, val, .. } => {
                let v = self.lower_expr_to_operand(val);
                Rvalue::UnaryOp { op: *op, val: v }
            }
            HirExpr::Call { callee, args, .. } => {
                let callee = self.lower_callee(callee);
                let args = args.iter().map(|a| self.lower_expr_to_operand(a)).collect();
                Rvalue::Call { callee, args }
            }
            HirExpr::MethodCall { receiver, name, args, .. } => {
                let recv_ty = self.infer_expr_type(receiver);
                let struct_id = match &recv_ty {
                    Ty::Struct(id) => *id,
                    Ty::Ref { inner, .. } => match &**inner {
                        Ty::Struct(id) => *id,
                        _ => DefId(u32::MAX),
                    },
                    _ => DefId(u32::MAX),
                };
                let recv = self.lower_expr_to_operand(receiver);
                let mut all_args = vec![recv];
                for a in args { all_args.push(self.lower_expr_to_operand(a)); }
                Rvalue::Call {
                    callee: Callee::Method { struct_id, name: name.clone() },
                    args: all_args,
                }
            }
            HirExpr::FieldAccess { obj, field, .. } => {
                let o = self.lower_expr_to_local(obj);
                Rvalue::Field { obj: o, field: field.clone() }
            }
            HirExpr::Index { obj, idx, .. } => {
                let o = self.lower_expr_to_local(obj);
                let i = self.lower_expr_to_local(idx);
                Rvalue::Index { obj: o, idx: i }
            }
            HirExpr::Array(items, _) => {
                let items = items.iter().map(|i| self.lower_expr_to_operand(i)).collect();
                Rvalue::ArrayInit { items }
            }
            HirExpr::StructLit { struct_id, fields, .. } => {
                let fields = fields.iter()
                    .map(|(n, e)| (n.clone(), self.lower_expr_to_operand(e)))
                    .collect();
                Rvalue::StructInit { struct_id: *struct_id, fields }
            }
            HirExpr::Borrow { mutable, val, .. } => {
                let place = self.lower_expr_to_place(val);
                Rvalue::Ref { mutable: *mutable, place }
            }
            HirExpr::Block(b) => {
                self.lower_block(b);
                Rvalue::Use(Operand::Const(Const::Unit))
            }
            HirExpr::Match { val, arms, .. } => {
                // Pós-MVP: lowering exaustivo. Por ora, lowera scrutinee e
                // representa como chamada placeholder; codegen real virá.
                let _ = self.lower_expr_to_local(val);
                let _ = arms;
                Rvalue::Use(Operand::Const(Const::Unit))
            }
            HirExpr::Assign { target, value, .. } => {
                let place = self.lower_expr_to_place(target);
                let v = self.lower_expr_to_operand(value);
                self.push_stmt(Statement::Store {
                    place, value: v, span: 0..0,
                });
                Rvalue::Use(Operand::Const(Const::Unit))
            }
        }
    }

    fn lower_expr_to_local(&mut self, e: &HirExpr) -> LocalId {
        // Fast path: ident → local existente.
        if let HirExpr::Name { id, .. } = e {
            if let Some(l) = self.def_to_local.get(id).copied() {
                return l;
            }
        }
        let ty = self.infer_expr_type(e);
        let rvalue = self.lower_expr_into(e);
        let tmp = self.new_tmp(ty);
        self.push_stmt(Statement::Assign {
            local: tmp, rvalue, span: 0..0,
        });
        tmp
    }

    fn lower_expr_to_operand(&mut self, e: &HirExpr) -> Operand {
        match e {
            HirExpr::Int(v, _)   => Operand::Const(Const::Int(*v)),
            HirExpr::Float(v, _) => Operand::Const(Const::Float(*v)),
            HirExpr::Bool(v, _)  => Operand::Const(Const::Bool(*v)),
            HirExpr::Str(v, _)   => Operand::Const(Const::Str(v.clone())),
            HirExpr::Name { id, .. } => {
                if let Some(l) = self.def_to_local.get(id).copied() {
                    return Operand::Local(l);
                }
                Operand::Const(Const::Unit)
            }
            _ => Operand::Local(self.lower_expr_to_local(e)),
        }
    }

    fn lower_expr_to_place(&mut self, e: &HirExpr) -> Place {
        match e {
            HirExpr::Name { id, .. } => {
                let local = self.def_to_local.get(id).copied().unwrap_or(LocalId(u32::MAX));
                Place { local, projections: vec![] }
            }
            HirExpr::FieldAccess { obj, field, .. } => {
                let mut p = self.lower_expr_to_place(obj);
                p.projections.push(Projection::Field(field.clone()));
                p
            }
            HirExpr::Index { obj, idx, .. } => {
                let mut p = self.lower_expr_to_place(obj);
                let i = self.lower_expr_to_local(idx);
                p.projections.push(Projection::Index(i));
                p
            }
            HirExpr::SelfRef(_) => Place {
                local: LocalId(0), // `self` é sempre o primeiro parâmetro
                projections: vec![],
            },
            _ => Place {
                local: self.lower_expr_to_local(e),
                projections: vec![],
            },
        }
    }

    fn lower_callee(&self, c: &HirExpr) -> Callee {
        match c {
            HirExpr::Name { id, name, .. } => {
                // Se é uma fn global (não local), Callee::Fn(*id).
                if !self.def_to_local.contains_key(id) {
                    Callee::Fn(*id)
                } else {
                    // Local sendo chamado — pós-MVP (closures). Por ora trata
                    // como builtin para não quebrar lowering.
                    Callee::Builtin(name.clone())
                }
            }
            HirExpr::Builtin { name, .. } => Callee::Builtin(name.clone()),
            _ => Callee::Builtin("<unknown>".into()),
        }
    }

    fn infer_expr_type(&self, e: &HirExpr) -> Ty {
        // Reaproveita a função do typeck via shim — não roda checking de novo,
        // só descobre tipo recursivamente. Para o MVP, mantemos uma
        // implementação local conservadora.
        match e {
            HirExpr::Int(_, _)   => Ty::Int,
            HirExpr::Float(_, _) => Ty::Float,
            HirExpr::Bool(_, _)  => Ty::Bool,
            HirExpr::Str(_, _)   => Ty::Str,
            HirExpr::Name { id, .. } => {
                if let Some(l) = self.def_to_local.get(id) {
                    return self.locals[l.0 as usize].ty.clone();
                }
                (self.type_of_def)(*id).unwrap_or(Ty::Error)
            }
            HirExpr::SelfRef(_) => self.self_ty.clone().unwrap_or(Ty::Error),
            HirExpr::BinOp { op, left, .. } => {
                use BinOp::*;
                match op {
                    Eq | Neq | Lt | Gt | Lte | Gte | And | Or => Ty::Bool,
                    _ => self.infer_expr_type(left),
                }
            }
            HirExpr::UnaryOp { op, val, .. } => match op {
                UnaryOp::Not => Ty::Bool,
                UnaryOp::Neg => self.infer_expr_type(val),
            },
            HirExpr::Array(items, _) => {
                let inner = items.first().map(|i| self.infer_expr_type(i)).unwrap_or(Ty::Error);
                Ty::Array(Box::new(inner))
            }
            HirExpr::StructLit { struct_id, .. } => Ty::Struct(*struct_id),
            HirExpr::Borrow { mutable, val, .. } => Ty::Ref {
                mutable: *mutable,
                inner: Box::new(self.infer_expr_type(val)),
            },
            HirExpr::Block(_) | HirExpr::Assign { .. } => Ty::Void,
            HirExpr::Call { callee, args, .. } => {
                if let HirExpr::Name { id, .. } = callee.as_ref() {
                    if !self.def_to_local.contains_key(id) {
                        if let Some(ty) = (self.type_of_def)(*id) {
                            return ty;
                        }
                    }
                }
                if let HirExpr::Builtin { name, .. } = callee.as_ref() {
                    if let Some((_, ret)) = vex_typeck::builtin_signature(name) {
                        // `Any` no retorno: usar tipo do primeiro arg como
                        // heurística (cobre min/max/pop). Boa o suficiente
                        // para o MVP — typeck já validou consistência.
                        if matches!(ret, Ty::Any) {
                            if let Some(a) = args.first() {
                                return self.infer_expr_type(a);
                            }
                        }
                        return ret;
                    }
                }
                Ty::Error
            }
            HirExpr::FieldAccess { obj, field, .. } => {
                let obj_ty = self.infer_expr_type(obj);
                let struct_id = match &obj_ty {
                    Ty::Struct(id) => Some(*id),
                    Ty::Ref { inner, .. } => match &**inner {
                        Ty::Struct(id) => Some(*id),
                        _ => None,
                    },
                    _ => None,
                };
                let Some(sid) = struct_id else { return Ty::Error };
                self.struct_fields.get(&sid)
                    .and_then(|fs| fs.get(field))
                    .cloned()
                    .unwrap_or(Ty::Error)
            }
            HirExpr::MethodCall { receiver, name, .. } => {
                let recv_ty = self.infer_expr_type(receiver);
                let sid = match &recv_ty {
                    Ty::Struct(id) => *id,
                    Ty::Ref { inner, .. } => match &**inner {
                        Ty::Struct(id) => *id,
                        _ => return Ty::Error,
                    },
                    _ => return Ty::Error,
                };
                self.method_ret.get(&(sid, name.clone()))
                    .cloned()
                    .unwrap_or(Ty::Error)
            }
            HirExpr::Index { obj, .. } => {
                match self.infer_expr_type(obj) {
                    Ty::Array(inner) => *inner,
                    _ => Ty::Error,
                }
            }
            HirExpr::Match { .. } | HirExpr::Builtin { .. } => Ty::Error,
        }
    }
}
