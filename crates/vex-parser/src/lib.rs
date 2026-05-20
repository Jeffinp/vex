//! Vex parser — recursive descent + Pratt para expressões.
//!
//! Decisão arquitetural: parser hand-written, não gerado. Padrão adotado
//! por rustc, Ruff (v0.4+), Carbon e Zig. Ganhos: controle total de
//! error recovery, mensagens diagnósticas customizadas, performance
//! previsível. Custo: mais código que LALRPOP, aceito.
//!
//! ## Organização
//! - `cursor`: peek/bump/expect sobre stream de tokens
//! - `item`: top-level (fn, struct, impl, const, use)
//! - `stmt`: statements e blocos
//! - `expr`: expressões via Pratt (binding power)
//! - `ty`: tipos
//! - `error`: erros estruturados com span
//!
//! ## Uso
//! ```ignore
//! let source = std::fs::read_to_string("foo.vex")?;
//! let module = vex_parser::parse(&source)?;
//! ```

mod cursor;
mod error;
mod expr;
mod item;
mod stmt;
mod ty;

pub use error::{ParseError, Span};

use vex_ast::{Block, FnDecl, Item, Module, Stmt, Type};
use vex_lexer::{Token, tokenize};

use cursor::Cursor;

/// Parser do top-level com **script mode** — princípio de design da Vex:
/// arquivos podem misturar items (fn, struct, impl, const, use) com
/// statements/exprs no topo. Statements top-level viram corpo de um
/// `main()` sintético — assim `println("hi")` no topo é um programa
/// válido, igual Python.
///
/// Conflito: se o arquivo declara `fn main()` **e** tem statements no
/// topo, é erro — ambíguo qual deveria executar.
pub fn parse(source: &str) -> Result<Module, ParseError> {
    let tokens = tokenize(source);
    let mut cur = Cursor::new(tokens, source.len());
    let mut items = Vec::new();
    let mut script_stmts: Vec<Stmt> = Vec::new();
    let mut script_span_start: Option<usize> = None;
    let mut script_span_end: usize = 0;

    while !cur.is_eof() {
        if peek_is_item(&cur)? {
            items.push(item::parse_item(&mut cur)?);
        } else {
            let stmt = stmt::parse_stmt(&mut cur)?;
            let _ = cur.eat(&Token::Semi)?; // `;` opcional como dentro de blocos
            if script_span_start.is_none() {
                script_span_start = Some(stmt_start(&stmt));
            }
            script_span_end = stmt_end(&stmt);
            script_stmts.push(stmt);
        }
    }

    if !script_stmts.is_empty() {
        // Conflito: já existe `fn main` declarada.
        if let Some(existing) = items.iter().find_map(|i| match i {
            Item::Fn(f) if f.name == "main" => Some(f.span.clone()),
            _ => None,
        }) {
            return Err(ParseError::Unexpected {
                expected: "ou `fn main()` explícita, ou statements top-level (script mode)".into(),
                found: "ambos no mesmo arquivo".into(),
                span: existing,
            });
        }

        let span = script_span_start.unwrap_or(0)..script_span_end.max(source.len());
        let body = Block { stmts: script_stmts, span: span.clone() };
        items.push(Item::Fn(FnDecl {
            name: "main".into(),
            params: Vec::new(),
            ret_type: Type::Void,
            body,
            is_pub: false,
            is_comptime: false,
            span,
        }));
    }

    Ok(Module { items })
}

/// Heurística: o próximo token (pulando modificadores `pub`/`comptime`)
/// inicia um item de top-level?
fn peek_is_item(cur: &Cursor) -> Result<bool, ParseError> {
    let mut offset = 0;
    loop {
        match cur.peek_n(offset)?.map(|st| &st.token) {
            Some(Token::Pub) | Some(Token::Comptime) => offset += 1,
            Some(Token::Fn)
            | Some(Token::Struct)
            | Some(Token::Impl)
            | Some(Token::Const)
            | Some(Token::Use)
            | Some(Token::Mod)
            | Some(Token::Trait)
            | Some(Token::Enum) => return Ok(true),
            _ => return Ok(false),
        }
    }
}

fn stmt_start(s: &Stmt) -> usize {
    match s {
        Stmt::Let { span, .. }
        | Stmt::Return(_, span)
        | Stmt::If { span, .. }
        | Stmt::While { span, .. }
        | Stmt::For { span, .. }
        | Stmt::Break(span)
        | Stmt::Continue(span) => span.start,
        Stmt::Expr(e) => expr::expr_span(e).start,
    }
}

fn stmt_end(s: &Stmt) -> usize {
    match s {
        Stmt::Let { span, .. }
        | Stmt::Return(_, span)
        | Stmt::If { span, .. }
        | Stmt::While { span, .. }
        | Stmt::For { span, .. }
        | Stmt::Break(span)
        | Stmt::Continue(span) => span.end,
        Stmt::Expr(e) => expr::expr_span(e).end,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vex_ast::{BinOp, Expr, Item, Stmt, Type};

    fn parse_ok(src: &str) -> Module {
        parse(src).unwrap_or_else(|e| panic!("parse error: {e:?} em {:?}", e.span()))
    }

    #[test]
    fn parse_empty_module() {
        let m = parse_ok("");
        assert!(m.items.is_empty());
    }

    #[test]
    fn parse_hello_world() {
        let m = parse_ok(r#"fn main() -> void { println("hello") }"#);
        assert_eq!(m.items.len(), 1);
        let Item::Fn(f) = &m.items[0] else { panic!() };
        assert_eq!(f.name, "main");
        assert!(matches!(f.ret_type, Type::Void));
        assert_eq!(f.body.stmts.len(), 1);
    }

    #[test]
    fn parse_fib() {
        let src = include_str!("../../../examples/fib.vex");
        let m = parse_ok(src);
        assert_eq!(m.items.len(), 2);
    }

    #[test]
    fn parse_arith_precedence() {
        // 1 + 2 * 3  →  1 + (2 * 3)
        let m = parse_ok("fn t() -> int { return 1 + 2 * 3 }");
        let Item::Fn(f) = &m.items[0] else { panic!() };
        let Stmt::Return(Some(e), _) = &f.body.stmts[0] else { panic!() };
        let Expr::BinOp { op: BinOp::Add, right, .. } = e else { panic!("got {e:?}") };
        assert!(matches!(**right, Expr::BinOp { op: BinOp::Mul, .. }));
    }

    #[test]
    fn parse_comparison_chain_not_associated() {
        // Comparações têm mesma BP — parser monta da esquerda.
        // `a < b == c` vira `(a < b) == c`.
        let m = parse_ok("fn t() -> bool { return 1 < 2 == 3 }");
        let Item::Fn(f) = &m.items[0] else { panic!() };
        let Stmt::Return(Some(e), _) = &f.body.stmts[0] else { panic!() };
        let Expr::BinOp { op: BinOp::Eq, left, .. } = e else { panic!() };
        assert!(matches!(**left, Expr::BinOp { op: BinOp::Lt, .. }));
    }

    #[test]
    fn parse_unary_neg() {
        let m = parse_ok("fn t() -> int { return -5 }");
        let Item::Fn(f) = &m.items[0] else { panic!() };
        let Stmt::Return(Some(e), _) = &f.body.stmts[0] else { panic!() };
        assert!(matches!(e, Expr::UnaryOp { .. }));
    }

    #[test]
    fn parse_let_with_inferred_type() {
        let m = parse_ok("fn t() -> void { let x = 42 }");
        let Item::Fn(f) = &m.items[0] else { panic!() };
        let Stmt::Let { name, type_ann, value, mutable, .. } = &f.body.stmts[0] else { panic!() };
        assert_eq!(name, "x");
        assert!(type_ann.is_none());
        assert!(!mutable);
        assert!(matches!(value, Expr::Int(42, _)));
    }

    #[test]
    fn parse_let_mut() {
        let m = parse_ok("fn t() -> void { let mut x: int = 0 }");
        let Item::Fn(f) = &m.items[0] else { panic!() };
        let Stmt::Let { mutable: true, .. } = &f.body.stmts[0] else { panic!() };
    }

    #[test]
    fn parse_if_else() {
        let m = parse_ok("fn t() -> int { if true { return 1 } else { return 2 } }");
        let Item::Fn(f) = &m.items[0] else { panic!() };
        let Stmt::If { else_body: Some(_), .. } = &f.body.stmts[0] else { panic!() };
    }

    #[test]
    fn parse_while() {
        let m = parse_ok("fn t() -> void { while x < 10 { x = x + 1 } }");
        let Item::Fn(f) = &m.items[0] else { panic!() };
        assert!(matches!(f.body.stmts[0], Stmt::While { .. }));
    }

    #[test]
    fn parse_struct_decl() {
        let m = parse_ok("struct Ponto { x: float, y: float }");
        let Item::Struct(s) = &m.items[0] else { panic!() };
        assert_eq!(s.name, "Ponto");
        assert_eq!(s.fields.len(), 2);
    }

    #[test]
    fn parse_impl_with_method() {
        let m = parse_ok("impl Ponto { fn x_val(self) -> float { return self.x } }");
        let Item::Impl(i) = &m.items[0] else { panic!() };
        assert_eq!(i.target, "Ponto");
        assert_eq!(i.methods.len(), 1);
    }

    #[test]
    fn parse_struct_literal_expr() {
        let m = parse_ok("fn t() -> void { let p = Ponto { x: 1.0, y: 2.0 } }");
        let Item::Fn(f) = &m.items[0] else { panic!() };
        let Stmt::Let { value, .. } = &f.body.stmts[0] else { panic!() };
        assert!(matches!(value, Expr::StructLit { .. }));
    }

    #[test]
    fn parse_method_call_chain() {
        let m = parse_ok("fn t() -> void { a.b().c.d() }");
        let Item::Fn(f) = &m.items[0] else { panic!() };
        assert!(matches!(f.body.stmts[0], Stmt::Expr(Expr::MethodCall { .. })));
    }

    #[test]
    fn parse_array_literal_and_index() {
        let m = parse_ok("fn t() -> int { let xs = [1, 2, 3] return xs[0] }");
        let Item::Fn(f) = &m.items[0] else { panic!() };
        assert!(matches!(&f.body.stmts[0], Stmt::Let { .. }));
        assert!(matches!(&f.body.stmts[1], Stmt::Return(Some(Expr::Index { .. }), _)));
    }

    #[test]
    fn parse_const_decl() {
        let m = parse_ok("pub const PI: float = 3.14159");
        let Item::Const(c) = &m.items[0] else { panic!() };
        assert_eq!(c.name, "PI");
        assert!(c.is_pub);
    }

    #[test]
    fn parse_use_path() {
        let m = parse_ok("use std::io::println");
        let Item::Use(u) = &m.items[0] else { panic!() };
        assert_eq!(u.segments, vec!["std", "io", "println"]);
    }

    #[test]
    fn parse_match_expr() {
        let m = parse_ok(r#"fn t() -> str { return match x { 0 => "z", _ => "o" } }"#);
        let Item::Fn(f) = &m.items[0] else { panic!() };
        let Stmt::Return(Some(Expr::Match { arms, .. }), _) = &f.body.stmts[0] else { panic!() };
        assert_eq!(arms.len(), 2);
    }

    #[test]
    fn parse_match_range_pattern() {
        let m = parse_ok(r#"fn t() -> str { return match x { 1..10 => "p", _ => "g" } }"#);
        let Item::Fn(f) = &m.items[0] else { panic!() };
        let Stmt::Return(Some(Expr::Match { arms, .. }), _) = &f.body.stmts[0] else { panic!() };
        assert!(matches!(arms[0].pattern, vex_ast::Pattern::Range { .. }));
    }

    #[test]
    fn parse_ref_and_mut_ref() {
        let m = parse_ok("fn t(p: &int, q: &mut int) -> void { }");
        let Item::Fn(f) = &m.items[0] else { panic!() };
        assert!(matches!(f.params[0].ty, Type::Ref { mutable: false, .. }));
        assert!(matches!(f.params[1].ty, Type::Ref { mutable: true, .. }));
    }

    #[test]
    fn parse_else_if_chain() {
        let src = "fn t() -> int { if a { return 1 } else if b { return 2 } else { return 3 } }";
        let m = parse_ok(src);
        let Item::Fn(f) = &m.items[0] else { panic!() };
        let Stmt::If { else_body: Some(eb), .. } = &f.body.stmts[0] else { panic!() };
        // else if vira bloco com um único If statement dentro
        assert!(matches!(eb.stmts[0], Stmt::If { else_body: Some(_), .. }));
    }

    #[test]
    fn parse_error_missing_paren() {
        let r = parse("fn t( -> void { }");
        assert!(r.is_err());
    }

    #[test]
    fn parse_error_invalid_token_in_expr() {
        let r = parse("fn t() -> int { return @ }");
        assert!(r.is_err());
    }
}
