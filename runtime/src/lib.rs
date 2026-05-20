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

// ── Arrays heap-allocated ──────────────────────────────────────────────
//
// Layout em LLVM: fat pointer `{ ptr: i8*, len: i64 }` (16 bytes).
// `vex_array_alloc` retorna ponteiro raw que o codegen armazena no
// campo `ptr` da struct. Codegen calcula `elem_size` em tempo de
// compilação para evitar overhead de tabela de tipos no runtime.

/// Aloca `n_bytes` heap zerados. Aborta se ENOMEM (sem unwind no MVP).
///
/// Codegen passa `count * sizeof(elem)` como `n_bytes`.
#[no_mangle]
pub extern "C" fn vex_array_alloc(n_bytes: u64) -> *mut u8 {
    if n_bytes == 0 {
        // Layout::array proíbe size 0; retornamos sentinela não-null
        // para distinguir de OOM. Codegen trata len==0 separadamente.
        return core::ptr::NonNull::dangling().as_ptr();
    }
    let layout = match std::alloc::Layout::from_size_align(n_bytes as usize, 8) {
        Ok(l) => l,
        Err(_) => std::process::abort(),
    };
    let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
    if ptr.is_null() { std::process::abort(); }
    ptr
}

/// Aborta o programa com mensagem de erro de bounds. Chamado pelo
/// codegen quando `xs[i]` com `i < 0 || i >= len`. Tem `#[cold]` para
/// hint ao otimizador — o caminho não-erro é o hot path.
#[cold]
#[no_mangle]
pub extern "C" fn vex_bounds_panic(idx: i64, len: i64) -> ! {
    eprintln!("vex panic: index {idx} out of bounds for array of length {len}");
    std::process::abort();
}

/// Libera array previamente alocado por `vex_array_alloc`.
///
/// # Safety
/// `ptr` deve ter sido retornado por `vex_array_alloc` com o mesmo
/// `n_bytes`. Chamar com tamanho diferente é UB (mesmo que Rust GlobalAlloc).
#[no_mangle]
pub unsafe extern "C" fn vex_array_drop(ptr: *mut u8, n_bytes: u64) {
    if ptr.is_null() || n_bytes == 0 { return; }
    let layout = match std::alloc::Layout::from_size_align(n_bytes as usize, 8) {
        Ok(l) => l,
        Err(_) => return,
    };
    unsafe { std::alloc::dealloc(ptr, layout); }
}

// ── Matemática (built-ins) ──────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn vex_sqrt(x: f64) -> f64  { x.sqrt() }

#[no_mangle]
pub extern "C" fn vex_abs_f64(x: f64) -> f64 { x.abs() }

#[no_mangle]
pub extern "C" fn vex_abs_i64(x: i64) -> i64 { x.abs() }
