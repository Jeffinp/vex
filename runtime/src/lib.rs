//! Runtime nativo Vex — linkado em todo binário gerado pelo compilador.
//!
//! Exporta funções com C ABI consumidas pelo código LLVM IR. Mantém o
//! footprint mínimo: usa `libc`/`std::io` apenas para I/O primitivo.
//! Fase 6: I/O básico para `print`/`println`. Fase 5b adicionará
//! `vex_gen_check` e hooks de drop ASAP.

use std::io::Write;

#[no_mangle]
pub extern "C" fn vex_print_int(x: i64) {
    let mut out = std::io::stdout().lock();
    let _ = write!(out, "{x}");
}

#[no_mangle]
pub extern "C" fn vex_println_int(x: i64) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{x}");
}

#[no_mangle]
pub extern "C" fn vex_print_float(x: f64) {
    let mut out = std::io::stdout().lock();
    let _ = write!(out, "{x:?}");
}

#[no_mangle]
pub extern "C" fn vex_println_float(x: f64) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{x:?}");
}

#[no_mangle]
pub extern "C" fn vex_print_bool(x: bool) {
    let mut out = std::io::stdout().lock();
    let _ = write!(out, "{x}");
}

#[no_mangle]
pub extern "C" fn vex_println_bool(x: bool) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{x}");
}

/// Recebe ponteiro para C string null-terminada (UTF-8 puro).
///
/// # Safety
/// `ptr` deve apontar para sequência válida de bytes terminada em NUL.
#[no_mangle]
pub unsafe extern "C" fn vex_print_str(ptr: *const u8) {
    if ptr.is_null() { return; }
    let cstr = unsafe { core::ffi::CStr::from_ptr(ptr as *const i8) };
    let s = cstr.to_string_lossy();
    let mut out = std::io::stdout().lock();
    let _ = write!(out, "{s}");
}

/// # Safety
/// Ver `vex_print_str`.
#[no_mangle]
pub unsafe extern "C" fn vex_println_str(ptr: *const u8) {
    if ptr.is_null() { return; }
    let cstr = unsafe { core::ffi::CStr::from_ptr(ptr as *const i8) };
    let s = cstr.to_string_lossy();
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{s}");
}

/// Núcleo do mecanismo de generational references (Fase 5b).
/// Aborta se generations não baterem. Por ora, no-op para não bloquear
/// codegen — checagens reais entram quando 5b for implementada.
#[no_mangle]
pub extern "C" fn vex_gen_check(_gen_stored: u64, _gen_current: u64) {
    // TODO Fase 5b
}

// ── Matemática (built-ins) ──────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn vex_sqrt(x: f64) -> f64  { x.sqrt() }

#[no_mangle]
pub extern "C" fn vex_abs_f64(x: f64) -> f64 { x.abs() }

#[no_mangle]
pub extern "C" fn vex_abs_i64(x: i64) -> i64 { x.abs() }
