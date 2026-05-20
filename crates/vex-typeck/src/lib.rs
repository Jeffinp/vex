//! Type checker + inferência local Hindley-Milner para Vex.
//!
//! Implementação iniciada na Fase 3. Responsabilidades:
//! - inferência de tipos em `let x = expr` (sem anotação)
//! - validação de compatibilidade em operações binárias
//! - checagem de retornos de função vs assinatura
//! - validação de chamadas (aridade + tipos)
//! - validação de acesso a campos de struct
//! - integração com ownership/linear types (Fase 6)

#[derive(Debug, thiserror::Error)]
pub enum TypeError {
    #[error("tipos incompatíveis: esperado `{expected}`, encontrado `{found}`")]
    Mismatch { expected: String, found: String, span: std::ops::Range<usize> },

    #[error("variável `{name}` não declarada")]
    Unknown { name: String, span: std::ops::Range<usize> },
}
