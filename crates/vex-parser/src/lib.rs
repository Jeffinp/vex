//! Vex parser — recursive descent + Pratt para expressões.
//!
//! Implementação a ser desenvolvida na Fase 2.
//! Decisão arquitetural: parser hand-written (não generator) para máximo
//! controle de error recovery e mensagens diagnósticas — padrão adotado
//! por rustc, Ruff (após v0.4) e outros compiladores production-grade.

use vex_ast::Module;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("unexpected token at {span:?}: {msg}")]
    Unexpected { msg: String, span: std::ops::Range<usize> },
    #[error("unexpected end of input")]
    Eof,
}

pub fn parse(_source: &str) -> Result<Module, ParseError> {
    Err(ParseError::Eof)
}
