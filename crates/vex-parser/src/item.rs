//! Parsing de top-level items: fn, struct, impl, const, use.

use vex_ast::{ConstDecl, FnDecl, ImplBlock, Item, Param, StructDecl, Type, UsePath};
use vex_lexer::Token;

use crate::cursor::Cursor;
use crate::error::ParseError;
use crate::expr::parse_expr;
use crate::stmt::parse_block;
use crate::ty::parse_type;

pub fn parse_item(cur: &mut Cursor) -> Result<Item, ParseError> {
    let is_pub = cur.eat(&Token::Pub)?;
    let is_comptime = cur.eat(&Token::Comptime)?;

    let st = cur.peek()?.ok_or_else(|| ParseError::UnexpectedEofWith {
        msg: "esperava declaração (fn, struct, impl, const, use)".into(),
        span: cur.eof_span(),
    })?.clone();

    match st.token {
        Token::Fn     => parse_fn(cur, is_pub, is_comptime).map(Item::Fn),
        Token::Struct => parse_struct(cur, is_pub).map(Item::Struct),
        Token::Impl   => parse_impl(cur).map(Item::Impl),
        Token::Const  => parse_const(cur, is_pub).map(Item::Const),
        Token::Use    => parse_use(cur).map(Item::Use),
        _ => Err(ParseError::Unexpected {
            expected: "fn, struct, impl, const ou use".into(),
            found: format!("{:?}", st.token),
            span: st.span,
        }),
    }
}

fn parse_fn(cur: &mut Cursor, is_pub: bool, is_comptime: bool) -> Result<FnDecl, ParseError> {
    let kw = cur.expect(Token::Fn)?;
    let name_tok = cur.bump()?;
    let name = match name_tok.token {
        Token::Ident(n) => n,
        _ => return Err(ParseError::Unexpected {
            expected: "nome da função".into(),
            found: format!("{:?}", name_tok.token),
            span: name_tok.span,
        }),
    };

    cur.expect(Token::LParen)?;
    let mut params = Vec::new();
    if !matches!(cur.peek()?, Some(st) if matches!(st.token, Token::RParen)) {
        loop {
            params.push(parse_param(cur)?);
            if !cur.eat(&Token::Comma)? { break; }
        }
    }
    cur.expect(Token::RParen)?;

    let ret_type = if cur.eat(&Token::Arrow)? {
        parse_type(cur)?
    } else {
        Type::Void
    };

    let body = parse_block(cur)?;
    let span = kw.span.start..body.span.end;

    Ok(FnDecl { name, params, ret_type, body, is_pub, is_comptime, span })
}

fn parse_param(cur: &mut Cursor) -> Result<Param, ParseError> {
    let mutable = cur.eat(&Token::Mut)?;
    let name_tok = cur.bump()?;
    let (name, name_span) = match name_tok.token {
        // `self` é parâmetro especial sem tipo declarado
        Token::SelfLower => {
            return Ok(Param {
                name: "self".into(),
                ty: Type::Named("Self".into()),
                mutable,
                span: name_tok.span,
            });
        }
        Token::Ident(n) => (n, name_tok.span),
        _ => return Err(ParseError::Unexpected {
            expected: "nome de parâmetro".into(),
            found: format!("{:?}", name_tok.token),
            span: name_tok.span,
        }),
    };
    cur.expect(Token::Colon)?;
    let ty = parse_type(cur)?;
    Ok(Param { name, ty, mutable, span: name_span })
}

fn parse_struct(cur: &mut Cursor, is_pub: bool) -> Result<StructDecl, ParseError> {
    let kw = cur.expect(Token::Struct)?;
    let name_tok = cur.bump()?;
    let name = match name_tok.token {
        Token::Ident(n) => n,
        _ => return Err(ParseError::Unexpected {
            expected: "nome da struct".into(),
            found: format!("{:?}", name_tok.token),
            span: name_tok.span,
        }),
    };
    cur.expect(Token::LBrace)?;
    let mut fields = Vec::new();
    while !matches!(cur.peek()?, Some(st) if matches!(st.token, Token::RBrace)) {
        let fname_tok = cur.bump()?;
        let (fname, fspan) = match fname_tok.token {
            Token::Ident(n) => (n, fname_tok.span),
            _ => return Err(ParseError::Unexpected {
                expected: "nome de campo".into(),
                found: format!("{:?}", fname_tok.token),
                span: fname_tok.span,
            }),
        };
        cur.expect(Token::Colon)?;
        let fty = parse_type(cur)?;
        fields.push((fname, fty, fspan));
        if !cur.eat(&Token::Comma)? { break; }
    }
    let rb = cur.expect(Token::RBrace)?;
    Ok(StructDecl { name, fields, is_pub, span: kw.span.start..rb.span.end })
}

fn parse_impl(cur: &mut Cursor) -> Result<ImplBlock, ParseError> {
    let kw = cur.expect(Token::Impl)?;
    let name_tok = cur.bump()?;
    let target = match name_tok.token {
        Token::Ident(n) => n,
        _ => return Err(ParseError::Unexpected {
            expected: "nome de tipo".into(),
            found: format!("{:?}", name_tok.token),
            span: name_tok.span,
        }),
    };
    cur.expect(Token::LBrace)?;
    let mut methods = Vec::new();
    while !matches!(cur.peek()?, Some(st) if matches!(st.token, Token::RBrace)) {
        let is_pub = cur.eat(&Token::Pub)?;
        methods.push(parse_fn(cur, is_pub, false)?);
    }
    let rb = cur.expect(Token::RBrace)?;
    Ok(ImplBlock { target, methods, span: kw.span.start..rb.span.end })
}

fn parse_const(cur: &mut Cursor, is_pub: bool) -> Result<ConstDecl, ParseError> {
    let kw = cur.expect(Token::Const)?;
    let name_tok = cur.bump()?;
    let name = match name_tok.token {
        Token::Ident(n) => n,
        _ => return Err(ParseError::Unexpected {
            expected: "nome da constante".into(),
            found: format!("{:?}", name_tok.token),
            span: name_tok.span,
        }),
    };
    cur.expect(Token::Colon)?;
    let ty = parse_type(cur)?;
    cur.expect(Token::Eq)?;
    let value = parse_expr(cur)?;
    let value_span = crate::expr::expr_span(&value);
    Ok(ConstDecl { name, ty, value, is_pub, span: kw.span.start..value_span.end })
}

fn parse_use(cur: &mut Cursor) -> Result<UsePath, ParseError> {
    let kw = cur.expect(Token::Use)?;
    let mut segments = Vec::new();
    let first = cur.bump()?;
    let mut end_span = first.span.end;
    match first.token {
        Token::Ident(n) => segments.push(n),
        _ => return Err(ParseError::Unexpected {
            expected: "identificador".into(),
            found: format!("{:?}", first.token),
            span: first.span,
        }),
    }
    while cur.eat(&Token::ColonColon)? {
        let seg_tok = cur.bump()?;
        end_span = seg_tok.span.end;
        match seg_tok.token {
            Token::Ident(n) => segments.push(n),
            _ => return Err(ParseError::Unexpected {
                expected: "identificador".into(),
                found: format!("{:?}", seg_tok.token),
                span: seg_tok.span,
            }),
        }
    }
    Ok(UsePath { segments, span: kw.span.start..end_span })
}
