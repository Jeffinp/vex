//! Camada central de diagnóstico do compilador Vex.
//!
//! Padroniza erros sobre `miette` para que todas as fases (lexer, parser,
//! typeck, codegen) reportem da mesma forma — com spans, severities e
//! hints renderizados graficamente no terminal.

use miette::{Diagnostic, NamedSource, SourceSpan};

/// Erro genérico de compilação. Cada crate define seus próprios tipos
/// implementando `Diagnostic` e convertendo para este na borda.
#[derive(thiserror::Error, Debug, Diagnostic)]
#[error("{message}")]
pub struct VexError {
    pub message: String,
    // Renomeado de `source` para evitar conflito com thiserror,
    // que interpreta qualquer campo `source` como `Error::source()`.
    #[source_code]
    pub src: NamedSource<String>,
    #[label("{label}")]
    pub span: SourceSpan,
    pub label: String,
    #[help]
    pub hint: Option<String>,
}

pub fn span_from_range(r: std::ops::Range<usize>) -> SourceSpan {
    (r.start, r.end - r.start).into()
}

pub fn named_source(path: &str, content: String) -> NamedSource<String> {
    NamedSource::new(path, content)
}
