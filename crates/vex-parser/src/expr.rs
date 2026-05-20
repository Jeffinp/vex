//! Parsing de expressões via **Pratt** (operator-precedence parsing).
//!
//! Por que Pratt? Recursive-descent puro exige uma função por nível de
//! precedência (15+ no caso de Vex). Pratt unifica tudo num único loop
//! parametrizado por *binding power* (BP). É a técnica usada por rustc,
//! Carbon, Zig, e amplamente recomendada pelo Aleksey Kladov ("matklad").
//!
//! Ver: <https://matklad.github.io/2020/04/13/simple-but-powerful-pratt-parsing.html>

use smol_str::SmolStr;
use vex_ast::{BinOp, Expr, MatchArm, Pattern, UnaryOp};
use vex_lexer::Token;

use crate::cursor::Cursor;
use crate::error::ParseError;

/// Tabela de precedência. Maior valor = liga mais forte (consome antes).
///
/// Inspirada em Rust, com poucas exceções:
/// - sem operadores bit-a-bit explícitos no nível de expressão da v0.1
/// - `..`/`..=` reservados para uso futuro em patterns
fn infix_binding_power(tok: &Token) -> Option<(u8, u8)> {
    Some(match tok {
        Token::OrOr                 => (1, 2),
        Token::AndAnd               => (3, 4),
        Token::EqEq | Token::Neq    => (5, 6),
        Token::Lt | Token::Gt
            | Token::Lte | Token::Gte => (7, 8),
        Token::Plus | Token::Minus  => (9, 10),
        Token::Star | Token::Slash
            | Token::Percent        => (11, 12),
        _ => return None,
    })
}

/// Prefixos (unary): `-x`, `!x`. Não há `&x` aqui — referências são
/// tratadas como casos especiais para evitar ambiguidade com `&&`.
fn prefix_binding_power(tok: &Token) -> Option<((), u8)> {
    Some(match tok {
        Token::Minus | Token::Bang => ((), 15),
        _ => return None,
    })
}

/// Postfix: `expr(...)` chamada, `expr.field`, `expr[idx]`.
fn postfix_binding_power(tok: &Token) -> Option<(u8, ())> {
    Some(match tok {
        Token::LParen | Token::Dot | Token::LBracket => (17, ()),
        _ => return None,
    })
}

pub fn parse_expr(cur: &mut Cursor) -> Result<Expr, ParseError> {
    parse_expr_bp(cur, 0)
}

fn parse_expr_bp(cur: &mut Cursor, min_bp: u8) -> Result<Expr, ParseError> {
    let mut lhs = parse_atom(cur)?;

    while let Some(st) = cur.peek()? {
        // postfix primeiro (liga mais forte que infix)
        if let Some((l_bp, ())) = postfix_binding_power(&st.token) {
            if l_bp < min_bp { break; }
            lhs = parse_postfix(cur, lhs)?;
            continue;
        }

        // assignment é right-associative de menor precedência (BP 0)
        if matches!(st.token, Token::Eq) {
            if min_bp > 0 { break; }
            cur.bump()?;
            let value = parse_expr_bp(cur, 0)?;
            let span = expr_span(&lhs).start..expr_span(&value).end;
            lhs = Expr::Assign {
                target: Box::new(lhs),
                value: Box::new(value),
                span,
            };
            continue;
        }

        if let Some((l_bp, r_bp)) = infix_binding_power(&st.token) {
            if l_bp < min_bp { break; }
            let op_tok = cur.bump()?;
            let rhs = parse_expr_bp(cur, r_bp)?;
            let span = expr_span(&lhs).start..expr_span(&rhs).end;
            lhs = Expr::BinOp {
                op: bin_op_from_token(&op_tok.token).expect("checked above"),
                left: Box::new(lhs),
                right: Box::new(rhs),
                span,
            };
            continue;
        }

        break;
    }

    Ok(lhs)
}

fn parse_postfix(cur: &mut Cursor, lhs: Expr) -> Result<Expr, ParseError> {
    let st = cur.peek()?.expect("caller verificou postfix").clone();
    match st.token {
        Token::LParen => {
            cur.bump()?;
            let mut args = Vec::new();
            if !matches!(cur.peek()?, Some(st) if matches!(st.token, Token::RParen)) {
                loop {
                    args.push(parse_expr(cur)?);
                    if !cur.eat(&Token::Comma)? { break; }
                }
            }
            let rp = cur.expect(Token::RParen)?;
            let span = expr_span(&lhs).start..rp.span.end;
            Ok(Expr::Call { callee: Box::new(lhs), args, span })
        }
        Token::Dot => {
            cur.bump()?;
            let next = cur.bump()?;
            let name = match next.token {
                Token::Ident(n) => n,
                _ => return Err(ParseError::Unexpected {
                    expected: "identificador".into(),
                    found: format!("{:?}", next.token),
                    span: next.span,
                }),
            };
            // method call?
            if matches!(cur.peek()?, Some(st) if matches!(st.token, Token::LParen)) {
                cur.bump()?;
                let mut args = Vec::new();
                if !matches!(cur.peek()?, Some(st) if matches!(st.token, Token::RParen)) {
                    loop {
                        args.push(parse_expr(cur)?);
                        if !cur.eat(&Token::Comma)? { break; }
                    }
                }
                let rp = cur.expect(Token::RParen)?;
                let span = expr_span(&lhs).start..rp.span.end;
                Ok(Expr::MethodCall {
                    receiver: Box::new(lhs), name, args, span,
                })
            } else {
                let span = expr_span(&lhs).start..next.span.end;
                Ok(Expr::FieldAccess { obj: Box::new(lhs), field: name, span })
            }
        }
        Token::LBracket => {
            cur.bump()?;
            let idx = parse_expr(cur)?;
            let rb = cur.expect(Token::RBracket)?;
            let span = expr_span(&lhs).start..rb.span.end;
            Ok(Expr::Index { obj: Box::new(lhs), idx: Box::new(idx), span })
        }
        _ => unreachable!(),
    }
}

fn parse_atom(cur: &mut Cursor) -> Result<Expr, ParseError> {
    let st = cur.peek()?.ok_or_else(|| ParseError::UnexpectedEofWith {
        msg: "esperava expressão".into(),
        span: cur.eof_span(),
    })?.clone();

    // prefix unary
    if let Some(((), r_bp)) = prefix_binding_power(&st.token) {
        let op_tok = cur.bump()?;
        let rhs = parse_expr_bp(cur, r_bp)?;
        let span = op_tok.span.start..expr_span(&rhs).end;
        let op = match op_tok.token {
            Token::Minus => UnaryOp::Neg,
            Token::Bang => UnaryOp::Not,
            _ => unreachable!(),
        };
        return Ok(Expr::UnaryOp { op, val: Box::new(rhs), span });
    }

    match st.token {
        Token::Int(v)   => { cur.bump()?; Ok(Expr::Int(v, st.span)) }
        Token::Float(v) => { cur.bump()?; Ok(Expr::Float(v, st.span)) }
        Token::Str(s)   => { cur.bump()?; Ok(Expr::Str(s, st.span)) }
        Token::True     => { cur.bump()?; Ok(Expr::Bool(true, st.span)) }
        Token::False    => { cur.bump()?; Ok(Expr::Bool(false, st.span)) }
        Token::SelfLower => { cur.bump()?; Ok(Expr::SelfRef(st.span)) }

        Token::Ident(name) => {
            cur.bump()?;
            // struct literal? `Ident { field: expr, ... }`
            // Heurística: apenas se identifier começa com maiúscula E
            // próximo token é `{`. Evita ambiguidade com `if cond { body }`.
            let is_capital = name.chars().next().is_some_and(|c| c.is_ascii_uppercase());
            if is_capital && matches!(cur.peek()?, Some(st) if matches!(st.token, Token::LBrace)) {
                return parse_struct_literal(cur, name, st.span);
            }
            Ok(Expr::Ident(name, st.span))
        }

        Token::LParen => {
            cur.bump()?;
            let e = parse_expr(cur)?;
            cur.expect(Token::RParen)?;
            Ok(e)
        }

        Token::LBracket => parse_array_literal(cur),

        Token::Amp => {
            cur.bump()?;
            let mutable = cur.eat(&Token::Mut)?;
            let val = parse_expr_bp(cur, 15)?;
            let span = st.span.start..expr_span(&val).end;
            Ok(Expr::Ref { mutable, val: Box::new(val), span })
        }

        Token::Match => parse_match(cur),

        Token::LBrace => {
            let block = crate::stmt::parse_block(cur)?;
            Ok(Expr::Block(block))
        }

        _ => Err(ParseError::InvalidExpr { span: st.span }),
    }
}

fn parse_struct_literal(
    cur: &mut Cursor,
    name: SmolStr,
    name_span: crate::error::Span,
) -> Result<Expr, ParseError> {
    cur.expect(Token::LBrace)?;
    let mut fields = Vec::new();
    if !matches!(cur.peek()?, Some(st) if matches!(st.token, Token::RBrace)) {
        loop {
            let next = cur.bump()?;
            let field_name = match next.token {
                Token::Ident(n) => n,
                _ => return Err(ParseError::Unexpected {
                    expected: "nome de campo".into(),
                    found: format!("{:?}", next.token),
                    span: next.span,
                }),
            };
            cur.expect(Token::Colon)?;
            let val = parse_expr(cur)?;
            fields.push((field_name, val));
            if !cur.eat(&Token::Comma)? { break; }
        }
    }
    let rb = cur.expect(Token::RBrace)?;
    Ok(Expr::StructLit { name, fields, span: name_span.start..rb.span.end })
}

fn parse_array_literal(cur: &mut Cursor) -> Result<Expr, ParseError> {
    let lb = cur.expect(Token::LBracket)?;
    let mut items = Vec::new();
    if !matches!(cur.peek()?, Some(st) if matches!(st.token, Token::RBracket)) {
        loop {
            items.push(parse_expr(cur)?);
            if !cur.eat(&Token::Comma)? { break; }
        }
    }
    let rb = cur.expect(Token::RBracket)?;
    Ok(Expr::Array(items, lb.span.start..rb.span.end))
}

fn parse_match(cur: &mut Cursor) -> Result<Expr, ParseError> {
    let m = cur.expect(Token::Match)?;
    let val = parse_expr(cur)?;
    cur.expect(Token::LBrace)?;
    let mut arms = Vec::new();
    while !matches!(cur.peek()?, Some(st) if matches!(st.token, Token::RBrace)) {
        let pat = parse_pattern(cur)?;
        cur.expect(Token::FatArrow)?;
        let body = parse_expr(cur)?;
        let span = expr_span(&body).clone();
        arms.push(MatchArm { pattern: pat, body, span });
        if !cur.eat(&Token::Comma)? { break; }
    }
    let rb = cur.expect(Token::RBrace)?;
    Ok(Expr::Match {
        val: Box::new(val),
        arms,
        span: m.span.start..rb.span.end,
    })
}

fn parse_pattern(cur: &mut Cursor) -> Result<Pattern, ParseError> {
    let st = cur.bump()?;
    match st.token {
        Token::Int(v) => {
            // possível range: `1..10` ou `1..=10`
            if let Some(next) = cur.peek()? {
                if matches!(next.token, Token::DotDot | Token::DotDotEq) {
                    let inclusive = matches!(next.token, Token::DotDotEq);
                    cur.bump()?;
                    let hi_tok = cur.bump()?;
                    let hi = match hi_tok.token {
                        Token::Int(n) => n,
                        _ => return Err(ParseError::InvalidPattern { span: hi_tok.span }),
                    };
                    return Ok(Pattern::Range { lo: v, hi, inclusive });
                }
            }
            Ok(Pattern::Int(v))
        }
        Token::True => Ok(Pattern::Bool(true)),
        Token::False => Ok(Pattern::Bool(false)),
        Token::Str(s) => Ok(Pattern::Str(s)),
        Token::Ident(n) if n == "_" => Ok(Pattern::Wildcard),
        Token::Ident(n) => Ok(Pattern::Ident(n)),
        _ => Err(ParseError::InvalidPattern { span: st.span }),
    }
}

fn bin_op_from_token(t: &Token) -> Option<BinOp> {
    Some(match t {
        Token::Plus => BinOp::Add,
        Token::Minus => BinOp::Sub,
        Token::Star => BinOp::Mul,
        Token::Slash => BinOp::Div,
        Token::Percent => BinOp::Mod,
        Token::EqEq => BinOp::Eq,
        Token::Neq => BinOp::Neq,
        Token::Lt => BinOp::Lt,
        Token::Gt => BinOp::Gt,
        Token::Lte => BinOp::Lte,
        Token::Gte => BinOp::Gte,
        Token::AndAnd => BinOp::And,
        Token::OrOr => BinOp::Or,
        _ => return None,
    })
}

pub fn expr_span(e: &Expr) -> crate::error::Span {
    match e {
        Expr::Int(_, s)
        | Expr::Float(_, s)
        | Expr::Str(_, s)
        | Expr::Bool(_, s)
        | Expr::Ident(_, s)
        | Expr::Array(_, s)
        | Expr::SelfRef(s) => s.clone(),
        Expr::BinOp { span, .. }
        | Expr::UnaryOp { span, .. }
        | Expr::Call { span, .. }
        | Expr::MethodCall { span, .. }
        | Expr::FieldAccess { span, .. }
        | Expr::Index { span, .. }
        | Expr::StructLit { span, .. }
        | Expr::Match { span, .. }
        | Expr::Ref { span, .. }
        | Expr::Assign { span, .. } => span.clone(),
        Expr::Block(b) => b.span.clone(),
    }
}
