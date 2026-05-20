//! Code generation Vex → LLVM IR via `inkwell`.
//!
//! Fase 4. Pipeline:
//! `vex-mir` → este crate → arquivo `.o` → linker (lld) → binário.
//!
//! Para Windows: usa `--target x86_64-pc-windows-gnu` + `llvm-mingw` para
//! cross-compilar do WSL2 Linux para `.exe`.

#[derive(Debug, thiserror::Error)]
pub enum CodegenError {
    #[error("erro LLVM: {0}")]
    Llvm(String),
    #[error("target inválido: {0}")]
    InvalidTarget(String),
}

pub struct CodegenOptions {
    pub target_triple: Option<String>, // ex: "x86_64-pc-windows-gnu"
    pub opt_level: u8,                  // 0..=3
    pub emit_ir: bool,
}

impl Default for CodegenOptions {
    fn default() -> Self {
        Self { target_triple: None, opt_level: 2, emit_ir: false }
    }
}
