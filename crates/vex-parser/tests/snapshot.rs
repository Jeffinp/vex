//! Snapshot tests do parser sobre os arquivos em `examples/`.
//!
//! Cobertura visual da AST. Diffs em `cargo insta review`.

use vex_parser::parse;

fn snap(src: &str) -> String {
    match parse(src) {
        Ok(m) => format!("{m:#?}"),
        Err(e) => format!("ERROR {e:?}"),
    }
}

#[test]
fn snapshot_hello() {
    let src = include_str!("../../../examples/hello.vex");
    insta::assert_snapshot!("hello", snap(src));
}

#[test]
fn snapshot_fib() {
    let src = include_str!("../../../examples/fib.vex");
    insta::assert_snapshot!("fib", snap(src));
}

#[test]
fn snapshot_ponto() {
    let src = include_str!("../../../examples/ponto.vex");
    insta::assert_snapshot!("ponto", snap(src));
}
