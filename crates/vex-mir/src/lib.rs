//! MIR (Mid-level IR) — desugar de ownership + lowering para CFG.
//!
//! Fase 6. Aqui acontece:
//! - inserção de checks de generational references (estilo Vale)
//! - validação de linear types (estilo Austral) para recursos
//! - ASAP destruction (estilo Mojo) — drops após último uso
//! - construção do control-flow graph que alimenta codegen
