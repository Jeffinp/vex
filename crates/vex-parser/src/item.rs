//! Parsing de top-level items: fn, struct, impl, const, use.

use vex_ast::{ConstDecl, FnDecl, ImplBlock, Item, Param, StructDecl, Type, UsePath};
use vex_lexer::Token;

use crate::cursor::Cursor;
use crate::error::ParseError;
use crate::expr::parse_expr;
use crate::stmt::parse_block;
use crate::ty::parse_type;

/// Parser de um item top-level. Retorna `Vec<Item>` em vez de `Item`
/// porque `class Foo { campos + métodos }` se desdobra em dois items
/// (StructDecl + ImplBlock). Caso normal: vec de 1 elemento.
pub fn parse_item(cur: &mut Cursor) -> Result<Vec<Item>, ParseError> {
    let is_pub = cur.eat(&Token::Pub)?;
    let is_comptime = cur.eat(&Token::Comptime)?;

    let st = cur.peek()?.ok_or_else(|| ParseError::UnexpectedEofWith {
        msg: "esperava declaração (fn, struct, impl, const, use)".into(),
        span: cur.eof_span(),
    })?.clone();

    match st.token {
        Token::Fn     => parse_fn(cur, is_pub, is_comptime).map(|f| vec![Item::Fn(f)]),
        Token::Struct => parse_struct_or_class(cur, is_pub),
        Token::Impl   => parse_impl(cur).map(|i| vec![Item::Impl(i)]),
        Token::Const  => parse_const(cur, is_pub).map(|c| vec![Item::Const(c)]),
        Token::Use    => parse_use(cur).map(|u| vec![Item::Use(u)]),
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

/// Parser de `class Foo { ... }` (alias: `struct Foo { ... }`).
///
/// O corpo aceita **campos** (`nome: Tipo,`) e **métodos** (`def`/`fn`)
/// misturados. Métodos são extraídos e empacotados num `ImplBlock`
/// implícito — o caller recebe `[StructDecl, ImplBlock?]`. Princípio
/// Python: ergonomia primeiro, semântica preserva o modelo Rust (impl
/// separado existe e continua aceito para extensões pós-fato).
fn parse_struct_or_class(cur: &mut Cursor, is_pub: bool) -> Result<Vec<Item>, ParseError> {
    let kw = cur.expect(Token::Struct)?;
    let name_tok = cur.bump()?;
    let name = match name_tok.token {
        Token::Ident(n) => n,
        _ => return Err(ParseError::Unexpected {
            expected: "nome da struct/class".into(),
            found: format!("{:?}", name_tok.token),
            span: name_tok.span,
        }),
    };
    cur.expect(Token::LBrace)?;

    let mut fields = Vec::new();
    let mut methods: Vec<FnDecl> = Vec::new();
    let mut expecting_comma = false;

    while !matches!(cur.peek()?, Some(st) if matches!(st.token, Token::RBrace)) {
        // Permite vírgula trailing e vírgula opcional entre campos.
        if expecting_comma {
            let _ = cur.eat(&Token::Comma)?;
            expecting_comma = false;
            continue;
        }
        // Lookahead: `pub` ou `fn`/`def` marca método; `Ident :` marca campo.
        let mut offset = 0;
        let is_method = loop {
            match cur.peek_n(offset)?.map(|st| &st.token) {
                Some(Token::Pub) | Some(Token::Comptime) => offset += 1,
                Some(Token::Fn) => break true,
                _ => break false,
            }
        };

        if is_method {
            let method_pub = cur.eat(&Token::Pub)?;
            let method_comptime = cur.eat(&Token::Comptime)?;
            let mut fn_decl = parse_fn(cur, method_pub, method_comptime)?;
            // Métodos dentro de class: nome canonical preservado.
            // Ajustes futuros (auto-namespace) podem rewriting aqui.
            fn_decl.is_pub = method_pub;
            fn_decl.is_comptime = method_comptime;
            methods.push(fn_decl);
            continue;
        }

        // Campo: `nome: Tipo`
        let fname_tok = cur.bump()?;
        let (fname, fspan) = match fname_tok.token {
            Token::Ident(n) => (n, fname_tok.span),
            _ => return Err(ParseError::Unexpected {
                expected: "nome de campo ou `def`".into(),
                found: format!("{:?}", fname_tok.token),
                span: fname_tok.span,
            }),
        };
        cur.expect(Token::Colon)?;
        let fty = parse_type(cur)?;
        fields.push((fname, fty, fspan));
        // Aceita vírgula como separador entre campos; opcional antes
        // de um método ou de `}`.
        if !cur.eat(&Token::Comma)? {
            // Próximo precisa ser `}` ou método/campo na próxima iteração.
            expecting_comma = false;
        }
    }
    let rb = cur.expect(Token::RBrace)?;

    let mut items = Vec::with_capacity(2);
    items.push(Item::Struct(StructDecl {
        name: name.clone(),
        fields,
        is_pub,
        span: kw.span.start..rb.span.end,
    }));
    if !methods.is_empty() {
        items.push(Item::Impl(ImplBlock {
            target: name,
            methods,
            span: kw.span.start..rb.span.end,
        }));
    }
    Ok(items)
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
