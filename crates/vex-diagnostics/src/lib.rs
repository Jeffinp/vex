//! Camada central de diagnóstico do compilador Vex.
//!
//! Padroniza erros sobre `miette` para que todas as fases (lexer, parser,
//! typeck, codegen) reportem da mesma forma — com spans, severities e
//! hints renderizados graficamente no terminal.

use miette::{Diagnostic, SourceSpan};

/// Erro genérico de compilação. Cada crate define seus próprios tipos
/// implementando `Diagnostic` e convertendo para este na borda.
#[derive(thiserror::Error, Debug, Diagnostic)]
#[error("{message}")]
pub struct VexError {
    pub message: String,
    #[source_code]
    pub source: String,
    #[label("{label}")]
    pub span: SourceSpan,
    pub label: String,
    #[help]
    pub hint: Option<String>,
}

pub fn span_from_range(r: std::ops::Range<usize>) -> SourceSpan {
    (r.start, r.end - r.start).into()
}
