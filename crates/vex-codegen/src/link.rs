//! Linker — junta `.o` Vex + runtime `vex-runtime` + libs do sistema.
//!
//! Estratégia simples: invoca `clang` (ou `x86_64-w64-mingw32-gcc` para
//! cross-Windows) via subprocess. Permite reuso da configuração de
//! linker da toolchain — não tentamos chamar `lld` direto.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("linker `{linker}` falhou (exit {code}): {stderr}")]
    LinkerFailed { linker: String, code: i32, stderr: String },
    #[error("não foi possível invocar `{linker}`: {source}")]
    SpawnFailed {
        linker: String,
        #[source]
        source: std::io::Error,
    },
    #[error("runtime não encontrado em `{0}`")]
    RuntimeNotFound(PathBuf),
}

pub struct LinkOptions {
    /// Caminho para `.o` do programa.
    pub object: PathBuf,
    /// Caminho de saída do binário.
    pub output: PathBuf,
    /// Caminho para `libvex_runtime.a` (gerado em `target/.../libvex_runtime.a`).
    pub runtime_lib: PathBuf,
    /// Triple. `None` = host.
    pub target_triple: Option<String>,
}

pub fn link_object(opts: &LinkOptions) -> Result<(), LinkError> {
    if !opts.runtime_lib.exists() {
        return Err(LinkError::RuntimeNotFound(opts.runtime_lib.clone()));
    }

    let (linker, extra_flags) = pick_linker(opts.target_triple.as_deref());

    let mut cmd = Command::new(&linker);
    cmd.arg(&opts.object)
        .arg(&opts.runtime_lib)
        .arg("-o")
        .arg(&opts.output);

    for flag in extra_flags {
        cmd.arg(flag);
    }

    // Em Linux precisamos linkar pthread/dl/libm — `clang` resolve via
    // libstd. Em mingw, libstd é estática.
    if opts.target_triple.is_none() {
        cmd.arg("-lpthread").arg("-ldl").arg("-lm");
    }

    let output = cmd.output().map_err(|e| LinkError::SpawnFailed {
        linker: linker.clone(),
        source: e,
    })?;

    if !output.status.success() {
        return Err(LinkError::LinkerFailed {
            linker,
            code: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

fn pick_linker(target: Option<&str>) -> (String, Vec<&'static str>) {
    match target {
        Some(t) if t.contains("windows-gnu") => {
            ("x86_64-w64-mingw32-gcc".into(), vec!["-static"])
        }
        _ => {
            // Tenta `clang`, `clang-17`, depois `gcc` na ordem de preferência.
            for cand in &["clang", "clang-17", "gcc"] {
                if which(cand).is_some() {
                    return ((*cand).to_string(), vec![]);
                }
            }
            ("clang".into(), vec![])
        }
    }
}

fn which(prog: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let p = dir.join(prog);
        if p.is_file() { return Some(p); }
    }
    None
}

/// Procura o `libvex_runtime.a` no `target/` do workspace. Convenção:
/// `target/<profile>/libvex_runtime.a` ou similar.
#[allow(dead_code)] // exposto para callers externos; driver atualmente usa lookup próprio.
pub fn find_runtime_lib(target_dir: &Path, profile: &str) -> Option<PathBuf> {
    let cand = target_dir.join(profile).join("libvex_runtime.a");
    if cand.exists() { return Some(cand); }
    // Fallback para o caso de cross-target build.
    let cand2 = target_dir.join("debug").join("libvex_runtime.a");
    if cand2.exists() { return Some(cand2); }
    None
}
