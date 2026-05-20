//! HIR (High-level IR) — AST após resolução de nomes.
//!
//! Cada identificador vira um `DefId` único. Estrutura inspirada no HIR
//! do rustc, simplificada. Permite typeck operar sem se preocupar com
//! escopos lexicais.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DefId(pub u32);
