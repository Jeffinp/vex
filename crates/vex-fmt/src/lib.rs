//! Formatter opinativo (zero config) para Vex.
//!
//! Princípio: como `gofmt`/`rustfmt`, não há flags de estilo. Uma única
//! formatação canônica. Implementado Fase 7.

pub fn format(source: &str) -> String { source.to_string() }
