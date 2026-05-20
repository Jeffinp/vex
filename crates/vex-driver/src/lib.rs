//! Driver — orquestra pipeline completo de compilação Vex.
//!
//! Pipeline atual (Fases 1-2 implementadas):
//!   source.vex
//!     → vex-lexer    (Tokens)
//!     → vex-parser   (AST)
//!     → [futuro] name resolution (HIR)
//!     → [futuro] vex-typeck (HIR tipada)
//!     → [futuro] lowering (MIR)
//!     → [futuro] vex-codegen (LLVM IR → .o)
//!     → [futuro] linker (binário ou .exe)

use std::path::PathBuf;

use miette::{Diagnostic, NamedSource, Report, SourceSpan};
use vex_hir::ResolveError;
use vex_typeck::TypeError;

#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    #[error("falha de parsing")]
    Parse,
    #[error("falha de resolução de nomes")]
    Resolve,
    #[error("falha de type-check (não implementado)")]
    Typeck,
    #[error("falha de codegen (não implementado)")]
    Codegen,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub struct CompileRequest {
    pub source_path: PathBuf,
    pub output_path: PathBuf,
    pub target: Option<String>,
    pub opt_level: u8,
    pub check_only: bool,
}

/// Diagnóstico renderizável com `miette` (mensagem, fonte com nome, span).
#[derive(Debug, thiserror::Error, Diagnostic)]
#[error("{message}")]
struct PrettyError {
    message: String,
    #[source_code]
    src: NamedSource<String>,
    #[label("{label}")]
    span: SourceSpan,
    label: String,
    #[help]
    hint: Option<String>,
}

/// Executa o pipeline. Atualmente cobre lex + parse; demais fases retornam
/// erro estruturado.
pub fn compile(req: CompileRequest) -> Result<(), DriverError> {
    let source = std::fs::read_to_string(&req.source_path)?;
    let path_str = req.source_path.display().to_string();

    let module = match vex_parser::parse(&source) {
        Ok(m) => m,
        Err(e) => {
            render(&path_str, &source, e.span().clone(), &e.to_string(), "aqui", None);
            return Err(DriverError::Parse);
        }
    };

    let (hir, resolve_errs) = vex_hir::resolve(&module);
    if !resolve_errs.is_empty() {
        for e in &resolve_errs {
            let (label, hint) = resolve_hint(e);
            render(&path_str, &source, e.span().clone(), &e.to_string(), label, hint);
        }
        eprintln!("✗ {} — {} erro(s) de resolução", path_str, resolve_errs.len());
        return Err(DriverError::Resolve);
    }

    let type_errs = vex_typeck::check_module(&hir);
    if !type_errs.is_empty() {
        for e in &type_errs {
            let (label, hint) = typeck_hint(e);
            render(&path_str, &source, e.span().clone(), &e.to_string(), label, hint);
        }
        eprintln!("✗ {} — {} erro(s) de tipo", path_str, type_errs.len());
        return Err(DriverError::Typeck);
    }

    if req.check_only {
        eprintln!(
            "✓ {} — lex + parse + resolve + typeck OK ({} item{}, {} defs)",
            path_str,
            hir.items.len(),
            if hir.items.len() == 1 { "" } else { "s" },
            hir.defs.len(),
        );
        return Ok(());
    }

    // Codegen ainda não implementado
    let _ = (req.output_path, req.target, req.opt_level);
    eprintln!(
        "⚠  {} type-check OK, mas codegen ainda não implementado (Fases 5-6).",
        path_str
    );
    Err(DriverError::Codegen)
}

fn render(
    path: &str,
    source: &str,
    span: std::ops::Range<usize>,
    message: &str,
    label: &str,
    hint: Option<&str>,
) {
    let span_len = span.end.saturating_sub(span.start).max(1);
    let pretty = PrettyError {
        message: message.to_string(),
        src: NamedSource::new(path, source.to_string()),
        span: (span.start, span_len).into(),
        label: label.to_string(),
        hint: hint.map(String::from),
    };
    let report: Report = pretty.into();
    eprintln!("{report:?}");
}

fn typeck_hint(e: &TypeError) -> (&'static str, Option<&'static str>) {
    match e {
        TypeError::Mismatch { .. } => (
            "tipo errado",
            Some("ajuste a anotação de tipo ou converta o valor"),
        ),
        TypeError::BadBinOp { .. } => (
            "operador incompatível",
            Some("verifique se ambos os lados têm o mesmo tipo numérico/bool"),
        ),
        TypeError::BadUnaryOp { .. } => (
            "operador unário inválido",
            Some("`-` requer numérico, `!` requer bool"),
        ),
        TypeError::BadArity { .. } => (
            "número errado de argumentos",
            Some("ajuste a chamada para bater com a assinatura da função"),
        ),
        TypeError::NotCallable { .. } => (
            "não é função",
            Some("apenas funções, métodos e built-ins podem ser chamados"),
        ),
        TypeError::UnknownField { .. } => (
            "campo inexistente",
            Some("verifique a declaração da struct"),
        ),
        TypeError::UnknownMethod { .. } => (
            "método inexistente",
            Some("declare o método em um `impl` ou verifique o tipo do receptor"),
        ),
        TypeError::BadReturn { .. } => (
            "retorno errado",
            Some("o valor retornado precisa bater com o tipo declarado na fn"),
        ),
        TypeError::NonBoolCond { .. } => (
            "não é bool",
            Some("condições precisam ter tipo bool"),
        ),
        TypeError::NonIntIndex { .. } => (
            "índice precisa ser int",
            Some("use uma expressão `int` dentro de `[...]`"),
        ),
        TypeError::NotIndexable { .. } => (
            "não é array",
            Some("apenas arrays podem ser indexados com `[i]`"),
        ),
        TypeError::NoFields { .. } => (
            "sem campos",
            Some("apenas structs têm campos acessíveis com `.`"),
        ),
        TypeError::MissingField { .. } => (
            "campo faltando",
            Some("forneça todos os campos da struct no literal"),
        ),
        TypeError::ExtraField { .. } => (
            "campo extra",
            Some("remova campos que não existem na declaração da struct"),
        ),
    }
}

fn resolve_hint(e: &ResolveError) -> (&'static str, Option<&'static str>) {
    match e {
        ResolveError::Unknown { .. } => (
            "não declarada",
            Some("declare com `let` antes de usar, ou verifique se há erro de digitação"),
        ),
        ResolveError::Duplicate { .. } => (
            "redeclarada aqui",
            Some("escolha outro nome ou remova a declaração duplicada"),
        ),
        ResolveError::UnknownType { .. } => (
            "tipo não encontrado",
            Some("declare a struct ou importe o tipo correto"),
        ),
        ResolveError::UnknownStruct { .. } => (
            "struct não declarada",
            Some("verifique se a struct existe no escopo"),
        ),
        ResolveError::SelfOutsideMethod { .. } => (
            "uso de `self` aqui",
            Some("`self` só funciona dentro de métodos em `impl` blocks"),
        ),
        ResolveError::ImplOnUnknownType { .. } => (
            "tipo não declarado",
            Some("declare a struct antes do `impl`"),
        ),
        ResolveError::InvalidAssignTarget { .. } => (
            "alvo inválido",
            Some("o lado esquerdo de `=` precisa ser uma variável, campo ou index"),
        ),
    }
}

