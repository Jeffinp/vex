//! Driver — orquestra pipeline completo de compilação Vex.
//!
//! source.vex
//!   → vex-lexer    (Tokens)
//!   → vex-parser   (AST)
//!   → name resolution (HIR)
//!   → vex-typeck   (HIR tipada)
//!   → lowering     (MIR)
//!   → vex-codegen  (LLVM IR → .o)
//!   → linker       (binário ou .exe)

#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    #[error("falha de parsing")]
    Parse,
    #[error("falha de type-check")]
    Typeck,
    #[error("falha de codegen")]
    Codegen,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub struct CompileRequest {
    pub source_path: std::path::PathBuf,
    pub output_path: std::path::PathBuf,
    pub target: Option<String>,
    pub opt_level: u8,
    pub check_only: bool,
}

pub fn compile(_req: CompileRequest) -> Result<(), DriverError> {
    // implementação progressiva ao longo das Fases 1-5
    Ok(())
}
