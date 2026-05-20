//! MIR (Mid-level IR) — control-flow graph para Vex.
//!
//! O MIR fica entre o HIR (árvore aninhada, expressivo) e o LLVM IR
//! (instruções planas com basic blocks). Aqui:
//! - Expressões aninhadas viram sequências de assignments em locals.
//! - Control flow vira CFG explícito com basic blocks + terminators.
//! - Cada local recebe um `LocalId` e um `Ty` (do typeck).
//!
//! **Não cobre ainda:** generational reference checks runtime + linear
//! type validation. Isso é Fase 5b — depende de análise de movimentos
//! sobre o CFG, que naturalmente vive em cima desta estrutura.
//!
//! Inspirado no MIR do rustc, drasticamente simplificado.

mod mir;
mod lower;
mod pretty;

pub use mir::*;
pub use lower::{lower_module, LowerError};
pub use pretty::pretty_print_module;
