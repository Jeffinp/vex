//! Type checker para Vex (Fase 4).
//!
//! Estratégia: **bidirecional simples + inferência local**.
//! - Top-down quando há tipo esperado (return, anotação de let, args de call).
//! - Bottom-up para inferir tipo de expressões livres.
//! - Sem unification: tipos primitivos batem por igualdade estrutural; não
//!   há generics no MVP. Generics monomorfizados ficam para v1.2+.
//!
//! Não é Hindley-Milner completo. É suficiente para Vex v0.1 e
//! drasticamente mais simples de implementar/manter.

mod ty;
mod env;
mod check;

pub use ty::{lower_hir_type, builtin_signature, Ty};
pub use check::{check_module, TypeError};
