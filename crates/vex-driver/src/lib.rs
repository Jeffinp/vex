//! Driver — orquestra pipeline completo de compilação Vex.
//!
//! Pipeline atual (Fases 1-2 implementadas):
//!   source.vex
//!     → vex-lexer    (Tokens)
//!     → vex-parser   (AST)
//!     → [futuro] name resolution (HIR)
//!     → [futuro] vex-typeck (HIR tipada)
//!     → [futuro] lowering (MIR)
//!     → [futuro] vex-codegen (LLVM IR → .o)
//!     → [futuro] linker (binário ou .exe)

use std::path::PathBuf;

use miette::{Diagnostic, NamedSource, Report, SourceSpan};
use vex_parser::ParseError;

#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    #[error("falha de parsing")]
    Parse,
    #[error("falha de type-check (não implementado)")]
    Typeck,
    #[error("falha de codegen (não implementado)")]
    Codegen,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub struct CompileRequest {
    pub source_path: PathBuf,
    pub output_path: PathBuf,
    pub target: Option<String>,
    pub opt_level: u8,
    pub check_only: bool,
}

/// Diagnóstico renderizável com `miette` (mensagem, fonte com nome, span).
#[derive(Debug, thiserror::Error, Diagnostic)]
#[error("{message}")]
struct PrettyParseError {
    message: String,
    #[source_code]
    src: NamedSource<String>,
    #[label("aqui")]
    span: SourceSpan,
}

/// Executa o pipeline. Atualmente cobre lex + parse; demais fases retornam
/// erro estruturado.
pub fn compile(req: CompileRequest) -> Result<(), DriverError> {
    let source = std::fs::read_to_string(&req.source_path)?;
    let path_str = req.source_path.display().to_string();

    let module = match vex_parser::parse(&source) {
        Ok(m) => m,
        Err(e) => {
            print_parse_error(&path_str, &source, &e);
            return Err(DriverError::Parse);
        }
    };

    if req.check_only {
        eprintln!(
            "✓ {} — parsing OK ({} item{})",
            path_str,
            module.items.len(),
            if module.items.len() == 1 { "" } else { "s" }
        );
        return Ok(());
    }

    // Fases ainda não implementadas
    let _ = (req.output_path, req.target, req.opt_level);
    eprintln!(
        "⚠  {} parseado, mas type-check/codegen ainda não implementados \
        (Fases 3-6).",
        path_str
    );
    Err(DriverError::Typeck)
}

fn print_parse_error(path: &str, source: &str, err: &ParseError) {
    let span = err.span();
    let span_len = span.end.saturating_sub(span.start).max(1);
    let pretty = PrettyParseError {
        message: err.to_string(),
        src: NamedSource::new(path, source.to_string()),
        span: (span.start, span_len).into(),
    };
    let report: Report = pretty.into();
    eprintln!("{report:?}");
}
