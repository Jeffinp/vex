//! Testes de lowering HIR → MIR.

use vex_hir::resolve;
use vex_mir::{lower_module, pretty_print_module, Terminator};
use vex_parser::parse;

fn lower(src: &str) -> vex_mir::MirModule {
    let ast = parse(src).expect("parse ok");
    let (hir, rerrs) = resolve(&ast);
    assert!(rerrs.is_empty(), "resolve errors: {rerrs:?}");
    let tyerrs = vex_typeck::check_module(&hir);
    assert!(tyerrs.is_empty(), "type errors: {tyerrs:?}");
    lower_module(&hir, &|_| None).expect("lowering ok")
}

#[test]
fn lower_empty_module() {
    let m = lower("");
    assert!(m.fns.is_empty());
    assert!(m.structs.is_empty());
}

#[test]
fn lower_simple_fn() {
    let m = lower("fn main() -> void { }");
    assert_eq!(m.fns.len(), 1);
    let f = &m.fns[0];
    assert_eq!(f.params.len(), 0);
    assert_eq!(f.blocks.len(), 1);
    assert!(matches!(f.blocks[0].terminator, Terminator::Return(None)));
}

#[test]
fn lower_fn_with_let_and_return() {
    let m = lower("fn t() -> int { let x = 1 + 2 return x }");
    let f = &m.fns[0];
    // Pelo menos um bloco, terminator Return(Some(_))
    let has_return = f.blocks.iter().any(|b|
        matches!(b.terminator, Terminator::Return(Some(_))));
    assert!(has_return, "missing Return(Some(_)) in MIR");
    // Algum stmt deve ser uma BinaryOp
    let has_binop = f.blocks.iter()
        .flat_map(|b| &b.stmts)
        .any(|s| matches!(s, vex_mir::Statement::Assign {
            rvalue: vex_mir::Rvalue::BinaryOp { .. }, ..
        }));
    assert!(has_binop, "missing BinaryOp in MIR");
}

#[test]
fn lower_if_creates_branches() {
    let m = lower("fn t() -> int { if true { return 1 } else { return 2 } }");
    let f = &m.fns[0];
    let has_if = f.blocks.iter().any(|b|
        matches!(b.terminator, Terminator::If { .. }));
    assert!(has_if, "missing If terminator");
}

#[test]
fn lower_while_creates_loop_back_edge() {
    let m = lower("fn t() -> void { while false { } }");
    let f = &m.fns[0];
    // Espera-se: entry → head → body → head (back edge) → exit
    let goto_back_edges = f.blocks.iter().filter(|b|
        matches!(b.terminator, Terminator::Goto(_))).count();
    assert!(goto_back_edges >= 2, "missing back edges in while loop");
}

#[test]
fn lower_fib_example() {
    let src = include_str!("../../../examples/fib.vex");
    let m = lower(src);
    assert_eq!(m.fns.len(), 2);
}

#[test]
fn lower_ponto_with_impl() {
    let src = include_str!("../../../examples/ponto.vex");
    let m = lower(src);
    // 1 struct, 1 método (distancia) + 1 main = 2 fns
    assert_eq!(m.structs.len(), 1);
    assert_eq!(m.fns.len(), 2);
}

#[test]
fn pretty_print_does_not_panic() {
    let src = include_str!("../../../examples/fib.vex");
    let m = lower(src);
    let p = pretty_print_module(&m);
    assert!(p.contains("fn"));
    assert!(p.contains("bb"));
}

#[test]
fn lower_call_emits_call_rvalue() {
    let m = lower("fn helper() -> int { return 1 } fn t() -> int { return helper() }");
    let has_call = m.fns.iter()
        .flat_map(|f| &f.blocks)
        .flat_map(|b| &b.stmts)
        .any(|s| matches!(s, vex_mir::Statement::Assign {
            rvalue: vex_mir::Rvalue::Call { .. }, ..
        }));
    assert!(has_call, "missing Call rvalue");
}

#[test]
fn lower_struct_init() {
    let src = "struct P { x: int } fn t() -> P { return P { x: 1 } }";
    let m = lower(src);
    let has_struct_init = m.fns.iter()
        .flat_map(|f| &f.blocks)
        .flat_map(|b| &b.stmts)
        .any(|s| matches!(s, vex_mir::Statement::Assign {
            rvalue: vex_mir::Rvalue::StructInit { .. }, ..
        }));
    assert!(has_struct_init);
}
