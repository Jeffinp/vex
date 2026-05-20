//! HIR (High-level IR) — AST após resolução de nomes.
//!
//! Cada identificador na AST original (`String`) vira um [`DefId`] único.
//! O HIR também é a primeira IR onde a árvore é **validada estruturalmente**:
//! variáveis usadas antes de declarar viram erro de resolução; campos de
//! struct referenciados em literais são checados (existência), etc.
//!
//! Estrutura inspirada no HIR do rustc, fortemente simplificada para Vex.
//!
//! Pipeline:
//!   AST (`vex-ast`) → [`resolve`] → HIR (este crate)
//!
//! Próxima fase consome o HIR para inferência de tipos (`vex-typeck`).

mod hir;
mod resolve;

pub use hir::*;
pub use resolve::{resolve, ResolveError};
