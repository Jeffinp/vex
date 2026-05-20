//! Parsing de statements e blocos.

use vex_ast::{Block, Stmt};
use vex_lexer::Token;

use crate::cursor::Cursor;
use crate::error::ParseError;
use crate::expr::{parse_expr, expr_span};

pub fn parse_block(cur: &mut Cursor) -> Result<Block, ParseError> {
    let lb = cur.expect(Token::LBrace)?;
    let mut stmts = Vec::new();
    while !matches!(cur.peek()?, Some(st) if matches!(st.token, Token::RBrace)) {
        stmts.push(parse_stmt(cur)?);
        // ponto-e-vírgula é opcional (estilo Rust/Go) — apenas consumido se presente.
        let _ = cur.eat(&Token::Semi)?;
    }
    let rb = cur.expect(Token::RBrace)?;
    Ok(Block { stmts, span: lb.span.start..rb.span.end })
}

pub fn parse_stmt(cur: &mut Cursor) -> Result<Stmt, ParseError> {
    let st = cur.peek()?.ok_or_else(|| ParseError::UnexpectedEofWith {
        msg: "esperava statement".into(),
        span: cur.eof_span(),
    })?.clone();

    match st.token {
        Token::Let      => parse_let(cur),
        Token::Return   => parse_return(cur),
        Token::If       => parse_if_stmt(cur),
        Token::While    => parse_while(cur),
        Token::For      => parse_for(cur),
        Token::Break    => { cur.bump()?; Ok(Stmt::Break(st.span)) }
        Token::Continue => { cur.bump()?; Ok(Stmt::Continue(st.span)) }
        _ => Ok(Stmt::Expr(parse_expr(cur)?)),
    }
}

fn parse_let(cur: &mut Cursor) -> Result<Stmt, ParseError> {
    let kw = cur.expect(Token::Let)?;
    let mutable = cur.eat(&Token::Mut)?;
    let name_tok = cur.bump()?;
    let name = match name_tok.token {
        Token::Ident(n) => n,
        _ => return Err(ParseError::Unexpected {
            expected: "identificador".into(),
            found: format!("{:?}", name_tok.token),
            span: name_tok.span,
        }),
    };
    let type_ann = if cur.eat(&Token::Colon)? {
        Some(crate::ty::parse_type(cur)?)
    } else {
        None
    };
    cur.expect(Token::Eq)?;
    let value = parse_expr(cur)?;
    let span = kw.span.start..expr_span(&value).end;
    Ok(Stmt::Let { name, mutable, type_ann, value, span })
}

fn parse_return(cur: &mut Cursor) -> Result<Stmt, ParseError> {
    let kw = cur.expect(Token::Return)?;
    // return sem valor: `return` seguido de `}` ou `;` ou EOF
    let no_value = match cur.peek()? {
        None => true,
        Some(st) => matches!(st.token, Token::RBrace | Token::Semi),
    };
    if no_value {
        return Ok(Stmt::Return(None, kw.span));
    }
    let value = parse_expr(cur)?;
    let span = kw.span.start..expr_span(&value).end;
    Ok(Stmt::Return(Some(value), span))
}

fn parse_if_stmt(cur: &mut Cursor) -> Result<Stmt, ParseError> {
    let kw = cur.expect(Token::If)?;
    let cond = parse_expr(cur)?;
    let then_body = parse_block(cur)?;
    let else_body = if cur.eat(&Token::Else)? {
        // suporta `else if ...` reentrante envolvendo em bloco
        if matches!(cur.peek()?, Some(st) if matches!(st.token, Token::If)) {
            let inner = parse_if_stmt(cur)?;
            let span = match &inner {
                Stmt::If { span, .. } => span.clone(),
                _ => unreachable!(),
            };
            Some(Block { stmts: vec![inner], span })
        } else {
            Some(parse_block(cur)?)
        }
    } else {
        None
    };
    let end = else_body.as_ref()
        .map(|b| b.span.end)
        .unwrap_or(then_body.span.end);
    let span = kw.span.start..end;
    Ok(Stmt::If { cond, then_body, else_body, span })
}

fn parse_while(cur: &mut Cursor) -> Result<Stmt, ParseError> {
    let kw = cur.expect(Token::While)?;
    let cond = parse_expr(cur)?;
    let body = parse_block(cur)?;
    let span = kw.span.start..body.span.end;
    Ok(Stmt::While { cond, body, span })
}

fn parse_for(cur: &mut Cursor) -> Result<Stmt, ParseError> {
    let kw = cur.expect(Token::For)?;
    let var_tok = cur.bump()?;
    let var = match var_tok.token {
        Token::Ident(n) => n,
        _ => return Err(ParseError::Unexpected {
            expected: "identificador".into(),
            found: format!("{:?}", var_tok.token),
            span: var_tok.span,
        }),
    };
    cur.expect(Token::In)?;
    let iter = parse_expr(cur)?;
    let body = parse_block(cur)?;
    let span = kw.span.start..body.span.end;
    Ok(Stmt::For { var, iter, body, span })
}
