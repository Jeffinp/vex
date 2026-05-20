//! Cursor sobre o stream de tokens.
//!
//! Encapsula peek/bump/expect e propaga `LexError`s do lexer como
//! [`ParseError::Lex`]. Toda lógica de "olhar o próximo token sem consumir"
//! passa por aqui — o parser nunca toca diretamente em índices.

use vex_lexer::{LexError, SpannedToken, Token};

use crate::error::ParseError;

pub type Span = std::ops::Range<usize>;

pub struct Cursor {
    tokens: Vec<Result<SpannedToken, LexError>>,
    pos: usize,
    /// Span virtual usado para erros de EOF (aponta para o fim do fonte).
    eof_span: Span,
}

impl Cursor {
    pub fn new(tokens: Vec<Result<SpannedToken, LexError>>, source_len: usize) -> Self {
        Self { tokens, pos: 0, eof_span: source_len..source_len }
    }

    /// Retorna o token atual sem consumir. `None` no EOF.
    pub fn peek(&self) -> Result<Option<&SpannedToken>, ParseError> {
        match self.tokens.get(self.pos) {
            None => Ok(None),
            Some(Ok(st)) => Ok(Some(st)),
            Some(Err(e)) => Err(ParseError::Lex(e.clone())),
        }
    }

    /// Olha N tokens à frente. Útil para distinguir construções com
    /// prefixo ambíguo (ex.: `Ident {` é struct literal? bloco?).
    #[allow(dead_code)]
    pub fn peek_n(&self, n: usize) -> Result<Option<&SpannedToken>, ParseError> {
        match self.tokens.get(self.pos + n) {
            None => Ok(None),
            Some(Ok(st)) => Ok(Some(st)),
            Some(Err(e)) => Err(ParseError::Lex(e.clone())),
        }
    }

    /// Consome o token atual.
    pub fn bump(&mut self) -> Result<SpannedToken, ParseError> {
        match self.tokens.get(self.pos) {
            None => Err(ParseError::UnexpectedEof { span: self.eof_span.clone() }),
            Some(Ok(st)) => {
                let st = st.clone();
                self.pos += 1;
                Ok(st)
            }
            Some(Err(e)) => {
                let e = e.clone();
                self.pos += 1;
                Err(ParseError::Lex(e))
            }
        }
    }

    /// Consome e exige um token específico (igualdade estrutural).
    pub fn expect(&mut self, expected: Token) -> Result<SpannedToken, ParseError> {
        let st = self.bump()?;
        if std::mem::discriminant(&st.token) == std::mem::discriminant(&expected) {
            Ok(st)
        } else {
            Err(ParseError::Unexpected {
                expected: format!("{expected:?}"),
                found: format!("{:?}", st.token),
                span: st.span,
            })
        }
    }

    /// Consome o token se ele bater com `expected`; retorna `true` se consumiu.
    pub fn eat(&mut self, expected: &Token) -> Result<bool, ParseError> {
        match self.peek()? {
            Some(st) if std::mem::discriminant(&st.token) == std::mem::discriminant(expected) => {
                self.bump()?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    pub fn eof_span(&self) -> Span {
        self.eof_span.clone()
    }
}
