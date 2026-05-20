//! Tipos do type checker. Análogo simplificado do `HirType`, mas resolvido
//! a um único shape uniforme (sem `Unresolved`/`SelfTy` no resultado final).

use std::fmt;
use smol_str::SmolStr;
use vex_hir::{DefId, HirType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    Int,
    Float,
    Bool,
    Str,
    Char,
    Void,
    Struct(DefId),
    Array(Box<Ty>),
    Ref { mutable: bool, inner: Box<Ty> },
    /// Aceita qualquer tipo. Usado em assinaturas de built-ins (ex.: `print`).
    Any,
    /// Tipo "erro" — propaga sem cascatear erros adicionais. Análogo ao
    /// `tcx.types.err` do rustc.
    Error,
}

impl Ty {
    pub fn is_numeric(&self) -> bool {
        matches!(self, Ty::Int | Ty::Float)
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Ty::Error)
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Int    => write!(f, "int"),
            Ty::Float  => write!(f, "float"),
            Ty::Bool   => write!(f, "bool"),
            Ty::Str    => write!(f, "str"),
            Ty::Char   => write!(f, "char"),
            Ty::Void   => write!(f, "void"),
            Ty::Struct(id) => write!(f, "struct#{}", id.0),
            Ty::Array(t)   => write!(f, "[{t}]"),
            Ty::Ref { mutable, inner } => {
                if *mutable { write!(f, "&mut {inner}") } else { write!(f, "&{inner}") }
            }
            Ty::Any   => write!(f, "<any>"),
            Ty::Error => write!(f, "<erro>"),
        }
    }
}

/// Converte HIR type para Ty. `self_ty` é necessário porque `HirType::SelfTy`
/// só faz sentido dentro de um impl block.
pub fn lower_hir_type(t: &HirType, self_ty: Option<&Ty>) -> Ty {
    match t {
        HirType::Int    => Ty::Int,
        HirType::Float  => Ty::Float,
        HirType::Bool   => Ty::Bool,
        HirType::Str    => Ty::Str,
        HirType::Char   => Ty::Char,
        HirType::Void   => Ty::Void,
        HirType::Struct(id) => Ty::Struct(*id),
        HirType::Array(inner) => Ty::Array(Box::new(lower_hir_type(inner, self_ty))),
        HirType::Ref { mutable, inner } => Ty::Ref {
            mutable: *mutable,
            inner: Box::new(lower_hir_type(inner, self_ty)),
        },
        HirType::SelfTy => self_ty.cloned().unwrap_or(Ty::Error),
        HirType::Unresolved(_) => Ty::Error,
    }
}

/// Igualdade de tipos para fins de checagem. `Any` é compatível com tudo;
/// `Error` propaga sem gerar novos erros.
pub fn unify(a: &Ty, b: &Ty) -> bool {
    match (a, b) {
        (Ty::Error, _) | (_, Ty::Error) => true,
        (Ty::Any, _)   | (_, Ty::Any)   => true,
        (Ty::Array(x), Ty::Array(y)) => unify(x, y),
        (Ty::Ref { mutable: m1, inner: i1 }, Ty::Ref { mutable: m2, inner: i2 }) => {
            m1 == m2 && unify(i1, i2)
        }
        _ => a == b,
    }
}

/// Built-in signatures. Polimorfismo limitado via `Any`.
pub fn builtin_signature(name: &SmolStr) -> Option<(Vec<Ty>, Ty)> {
    Some(match name.as_str() {
        "print"    | "println" => (vec![Ty::Any], Ty::Void),
        "input"    => (vec![Ty::Str], Ty::Str),
        "read_file" => (vec![Ty::Str], Ty::Str),
        "write_file" => (vec![Ty::Str, Ty::Str], Ty::Void),
        "to_int"   => (vec![Ty::Str], Ty::Int),
        "to_float" => (vec![Ty::Str], Ty::Float),
        "to_str"   => (vec![Ty::Any], Ty::Str),
        "len"      => (vec![Ty::Any], Ty::Int),     // Any aceita [T] ou str
        "push"     => (vec![Ty::Any, Ty::Any], Ty::Void),
        "pop"      => (vec![Ty::Any], Ty::Any),
        "sqrt" | "abs" => (vec![Ty::Float], Ty::Float),
        "min" | "max"  => (vec![Ty::Any, Ty::Any], Ty::Any),
        _ => return None,
    })
}
