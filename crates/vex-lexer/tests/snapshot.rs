//! Snapshot tests para o lexer.
//!
//! Tokeniza arquivos reais de `examples/` e congela o resultado em snapshots.
//! Regressões na tokenização aparecem como diffs em `cargo insta review`.

use vex_lexer::tokenize;

fn snapshot_of(src: &str) -> String {
    let mut out = String::new();
    for r in tokenize(src) {
        match r {
            Ok(st) => out.push_str(&format!("{:>4}..{:<4} {:?}\n", st.span.start, st.span.end, st.token)),
            Err(e) => out.push_str(&format!("ERROR {:?}\n", e)),
        }
    }
    out
}

#[test]
fn snapshot_hello() {
    let src = include_str!("../../../examples/hello.vex");
    insta::assert_snapshot!("hello", snapshot_of(src));
}

#[test]
fn snapshot_fib() {
    let src = include_str!("../../../examples/fib.vex");
    insta::assert_snapshot!("fib", snapshot_of(src));
}

#[test]
fn snapshot_ponto() {
    let src = include_str!("../../../examples/ponto.vex");
    insta::assert_snapshot!("ponto", snapshot_of(src));
}
