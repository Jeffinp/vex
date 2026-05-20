//! Parsing de tipos.

use vex_ast::Type;
use vex_lexer::Token;

use crate::cursor::Cursor;
use crate::error::ParseError;

pub fn parse_type(cur: &mut Cursor) -> Result<Type, ParseError> {
    let st = cur.peek()?.ok_or_else(|| ParseError::UnexpectedEofWith {
        msg: "esperava tipo".into(),
        span: cur.eof_span(),
    })?;

    match &st.token {
        Token::TInt   => { cur.bump()?; Ok(Type::Int) }
        Token::TFloat => { cur.bump()?; Ok(Type::Float) }
        Token::TBool  => { cur.bump()?; Ok(Type::Bool) }
        Token::TStr   => { cur.bump()?; Ok(Type::Str) }
        Token::TVoid  => { cur.bump()?; Ok(Type::Void) }
        Token::Ident(name) => {
            let name = name.clone();
            cur.bump()?;
            Ok(Type::Named(name))
        }
        Token::LBracket => {
            cur.bump()?;
            let inner = parse_type(cur)?;
            cur.expect(Token::RBracket)?;
            Ok(Type::Array(Box::new(inner)))
        }
        Token::Amp => {
            cur.bump()?;
            let mutable = cur.eat(&Token::Mut)?;
            let inner = parse_type(cur)?;
            Ok(Type::Ref { mutable, inner: Box::new(inner) })
        }
        _ => {
            let span = st.span.clone();
            Err(ParseError::InvalidType { span })
        }
    }
}
