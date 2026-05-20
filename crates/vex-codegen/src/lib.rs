//! Code generation Vex → LLVM IR via `inkwell`.
//!
//! Fase 6. Pipeline:
//! `vex-mir::MirModule` → este crate → arquivo `.o` → linker (clang/lld)
//! → binário.
//!
//! Estratégia:
//! - cada `MirFn` vira `FunctionValue` LLVM
//! - cada `MirLocal` vira `alloca` no bloco entry
//! - cada `BasicBlock` vira `BasicBlockValue`
//! - statements/rvalues lowered diretamente para instruções LLVM
//! - built-ins (print, println) mapeiam para externs do runtime
//!   (`vex_print_*`/`vex_println_*` em `runtime/`)
//!
//! Cross-compile Windows: passar `target_triple = Some("x86_64-pc-windows-gnu")`
//! em `CodegenOptions`; linkagem com `llvm-mingw` é tarefa do driver.

mod compile;
mod link;

pub use compile::{compile_module, CodegenError, CodegenOptions};
pub use link::{link_object, LinkError, LinkOptions};
