//! Runtime nativo Vex — linkado em todo binário gerado pelo compilador.
//!
//! Responsabilidades:
//! - alocação heap (tabela de generations para Vale-style refs)
//! - intrínsecos (`print`, `println`, conversões)
//! - panic handler (mensagens com span do fonte original)
//! - hooks de drop ASAP (Mojo-style)
//!
//! Exporta C ABI para ser chamado a partir do código LLVM IR gerado.

// Nota: `no_std` será reativado na Fase 6 quando o panic handler nativo
// estiver implementado. Por ora, usa std para facilitar setup.
#![allow(clippy::missing_safety_doc)]

// Stubs C ABI. Implementação real na Fase 6.

/// Imprime um inteiro 64-bit seguido de newline.
///
/// Chamado por `print(x: int)` em código Vex compilado.
#[no_mangle]
pub extern "C" fn vex_print_int(_x: i64) {
    // TODO Fase 6: implementar via syscall write(1, ...) sem libc para no_std.
}

/// Verifica que `gen_stored == gen_current`. Aborta com panic se diferentes.
///
/// Núcleo do mecanismo de generational references (estilo Vale).
#[no_mangle]
pub extern "C" fn vex_gen_check(_gen_stored: u64, _gen_current: u64) {
    // TODO Fase 6
}
