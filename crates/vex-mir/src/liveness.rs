//! Liveness analysis — fundação para ownership (Fase 5b).
//!
//! Computa para cada `BasicBlock`:
//! - `use_set`: locals lidos antes de qualquer escrita no bloco
//! - `def_set`: locals escritos no bloco
//! - `live_in`:  locals vivos na entrada do bloco
//! - `live_out`: locals vivos na saída do bloco
//!
//! Algoritmo clássico de dataflow backward (dragão book §9.2):
//! ```text
//! live_out[B] = ⋃ live_in[S] para todo sucessor S de B
//! live_in[B]  = use_set[B] ∪ (live_out[B] - def_set[B])
//! ```
//! Itera até atingir fixed-point.
//!
//! Saída deste pass alimenta:
//! - **ASAP destruction:** drop local no último ponto onde ele está vivo
//! - **Move analysis:** detectar use-after-move
//! - **Gen-ref insertion:** validar que referência ainda é válida
//!
//! O codegen ainda não consome estas anotações — esta é a infra; passos
//! seguintes da 5b emitem drops/checks com base nelas.

use indexmap::{IndexMap, IndexSet};

use crate::mir::{
    BlockId, Callee, LocalId, MirFn, Operand, Place, Rvalue, Statement, Terminator,
};

#[derive(Debug, Clone, Default)]
pub struct BlockLiveness {
    pub use_set: IndexSet<LocalId>,
    pub def_set: IndexSet<LocalId>,
    pub live_in: IndexSet<LocalId>,
    pub live_out: IndexSet<LocalId>,
}

#[derive(Debug, Clone)]
pub struct FnLiveness {
    /// Por `BlockId.0`.
    pub blocks: Vec<BlockLiveness>,
    /// Último bloco onde cada local está vivo. Útil para inserir drops.
    pub last_use: IndexMap<LocalId, BlockId>,
}

/// Roda liveness sobre uma fn. Retorna anotações por bloco + last_use.
pub fn analyze(f: &MirFn) -> FnLiveness {
    let mut blocks: Vec<BlockLiveness> = (0..f.blocks.len())
        .map(|_| BlockLiveness::default())
        .collect();

    // Pass 1: computa use_set e def_set por bloco.
    for b in &f.blocks {
        let bl = &mut blocks[b.id.0 as usize];
        for stmt in &b.stmts {
            collect_uses_defs(stmt, bl);
        }
        terminator_uses(&b.terminator, bl);
    }

    // Pass 2: dataflow backward até fixed-point.
    let mut changed = true;
    while changed {
        changed = false;
        for b in f.blocks.iter().rev() {
            let succs = successors(&b.terminator);
            let mut new_out = IndexSet::new();
            for s in &succs {
                for l in &blocks[s.0 as usize].live_in {
                    new_out.insert(*l);
                }
            }
            let mut new_in = blocks[b.id.0 as usize].use_set.clone();
            for l in &new_out {
                if !blocks[b.id.0 as usize].def_set.contains(l) {
                    new_in.insert(*l);
                }
            }
            let bl = &mut blocks[b.id.0 as usize];
            if new_in != bl.live_in {
                bl.live_in = new_in;
                changed = true;
            }
            if new_out != bl.live_out {
                bl.live_out = new_out;
                changed = true;
            }
        }
    }

    // Last-use: maior `BlockId` onde cada local é referenciado (lido ou
    // escrito) ou sai vivo. Aproximação por bloco — granular o suficiente
    // para ASAP destruction grosso.
    let mut last_use: IndexMap<LocalId, BlockId> = IndexMap::new();
    for b in &f.blocks {
        let bl = &blocks[b.id.0 as usize];
        for l in bl.use_set.iter()
            .chain(bl.def_set.iter())
            .chain(bl.live_out.iter())
        {
            last_use.insert(*l, b.id);
        }
    }

    FnLiveness { blocks, last_use }
}

fn successors(t: &Terminator) -> Vec<BlockId> {
    match t {
        Terminator::Goto(b) => vec![*b],
        Terminator::If { then, otherwise, .. } => vec![*then, *otherwise],
        Terminator::Return(_) | Terminator::Unreachable => vec![],
    }
}

fn collect_uses_defs(stmt: &Statement, bl: &mut BlockLiveness) {
    match stmt {
        Statement::Assign { local, rvalue, .. } => {
            rvalue_uses(rvalue, bl);
            if !bl.use_set.contains(local) {
                bl.def_set.insert(*local);
            }
        }
        Statement::Store { place, value, .. } => {
            place_uses(place, bl);
            operand_use(value, bl);
        }
        Statement::Nop => {}
    }
}

fn rvalue_uses(r: &Rvalue, bl: &mut BlockLiveness) {
    match r {
        Rvalue::Use(o) => operand_use(o, bl),
        Rvalue::BinaryOp { lhs, rhs, .. } => {
            operand_use(lhs, bl);
            operand_use(rhs, bl);
        }
        Rvalue::UnaryOp { val, .. } => operand_use(val, bl),
        Rvalue::Call { callee, args } => {
            for a in args { operand_use(a, bl); }
            match callee {
                Callee::Fn(_) | Callee::Method { .. } | Callee::Builtin(_) => {}
            }
        }
        Rvalue::Field { obj, .. } => add_use(*obj, bl),
        Rvalue::Index { obj, idx } => {
            add_use(*obj, bl);
            add_use(*idx, bl);
        }
        Rvalue::Ref { place, .. } => place_uses(place, bl),
        Rvalue::StructInit { fields, .. } => {
            for (_, op) in fields { operand_use(op, bl); }
        }
        Rvalue::ArrayInit { items } => {
            for op in items { operand_use(op, bl); }
        }
    }
}

fn operand_use(o: &Operand, bl: &mut BlockLiveness) {
    if let Operand::Local(l) = o {
        add_use(*l, bl);
    }
}

fn place_uses(p: &Place, bl: &mut BlockLiveness) {
    add_use(p.local, bl);
    for proj in &p.projections {
        if let crate::mir::Projection::Index(i) = proj {
            add_use(*i, bl);
        }
    }
}

fn add_use(l: LocalId, bl: &mut BlockLiveness) {
    if !bl.def_set.contains(&l) {
        bl.use_set.insert(l);
    }
}

fn terminator_uses(t: &Terminator, bl: &mut BlockLiveness) {
    match t {
        Terminator::If { cond, .. } => add_use(*cond, bl),
        Terminator::Return(Some(l)) => add_use(*l, bl),
        Terminator::Goto(_) | Terminator::Return(None) | Terminator::Unreachable => {}
    }
}

pub fn pretty_print(f: &MirFn, l: &FnLiveness) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    writeln!(out, "fn #{} {} liveness:", f.id.0, f.name).unwrap();
    for b in &f.blocks {
        let bl = &l.blocks[b.id.0 as usize];
        writeln!(out, "  bb{}:", b.id.0).unwrap();
        writeln!(out, "    use:      {}", set_str(&bl.use_set)).unwrap();
        writeln!(out, "    def:      {}", set_str(&bl.def_set)).unwrap();
        writeln!(out, "    live_in:  {}", set_str(&bl.live_in)).unwrap();
        writeln!(out, "    live_out: {}", set_str(&bl.live_out)).unwrap();
    }
    writeln!(out, "  last_use:").unwrap();
    for (l, b) in &l.last_use {
        writeln!(out, "    _{} → bb{}", l.0, b.0).unwrap();
    }
    out
}

fn set_str(s: &IndexSet<LocalId>) -> String {
    if s.is_empty() { return "∅".into(); }
    let mut v: Vec<_> = s.iter().map(|l| format!("_{}", l.0)).collect();
    v.sort();
    v.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use vex_hir::resolve;
    use vex_parser::parse;

    fn lower(src: &str) -> crate::mir::MirModule {
        let ast = parse(src).expect("parse ok");
        let (hir, _) = resolve(&ast);
        crate::lower::lower_module(&hir, &|_| None).expect("lowering ok")
    }

    #[test]
    fn live_simple_return() {
        let m = lower("fn t() -> int { let x = 1 return x }");
        let f = &m.fns[0];
        let liv = analyze(f);
        // Pelo menos um bloco tem `x` em live_out ou usa.
        assert!(liv.last_use.iter().any(|(l, _)| l.0 == 0 || l.0 == 1));
    }

    #[test]
    fn live_if_branches_merge() {
        let src = "fn t() -> int { let x = 1 if true { return x } else { return 2 } }";
        let m = lower(src);
        let f = &m.fns[0];
        let liv = analyze(f);
        // Pelo menos um bloco tem live_in não vazio (x propagado).
        assert!(liv.blocks.iter().any(|b| !b.live_in.is_empty()));
    }

    #[test]
    fn live_dead_local_has_no_uses() {
        // `let x = 1` sem usar x — x morre imediato.
        let m = lower("fn t() -> void { let x = 1 }");
        let f = &m.fns[0];
        let liv = analyze(f);
        // x não está em nenhum live_out.
        assert!(liv.blocks.iter().all(|b| b.live_out.is_empty()));
    }

    #[test]
    fn pretty_print_does_not_panic() {
        let m = lower("fn t() -> int { let x = 1 return x }");
        let liv = analyze(&m.fns[0]);
        let _ = pretty_print(&m.fns[0], &liv);
    }
}
