//! Erros de parsing.
//!
//! Cada variante carrega o span exato para que `vex-diagnostics` consiga
//! renderizar o erro graficamente com `miette`.

use vex_lexer::LexError;

pub type Span = std::ops::Range<usize>;

#[derive(Debug, thiserror::Error, Clone)]
pub enum ParseError {
    #[error("token inesperado: esperava `{expected}`, encontrou `{found}`")]
    Unexpected { expected: String, found: String, span: Span },

    #[error("fim de arquivo inesperado: {msg}")]
    UnexpectedEofWith { msg: String, span: Span },

    #[error("fim de arquivo inesperado")]
    UnexpectedEof { span: Span },

    #[error("expressão inválida")]
    InvalidExpr { span: Span },

    #[error("tipo inválido")]
    InvalidType { span: Span },

    #[error("padrão (pattern) inválido")]
    InvalidPattern { span: Span },

    #[error("erro de tokenização")]
    Lex(#[from] LexError),
}

impl ParseError {
    pub fn span(&self) -> &Span {
        match self {
            ParseError::Unexpected { span, .. }
            | ParseError::UnexpectedEofWith { span, .. }
            | ParseError::UnexpectedEof { span }
            | ParseError::InvalidExpr { span }
            | ParseError::InvalidType { span }
            | ParseError::InvalidPattern { span } => span,
            ParseError::Lex(e) => e.span(),
        }
    }
}
