//! `vex` — CLI principal da linguagem Vex.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use vex_driver::{CompileRequest, DriverError, EmitKind, compile};

#[derive(Parser)]
#[command(name = "vex", version, about = "Vex programming language compiler")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compila e executa um programa Vex.
    Run {
        file: PathBuf,
        #[arg(long, short = 'O', default_value_t = 2)]
        opt_level: u8,
    },
    /// Compila um programa Vex para um binário nativo.
    Build {
        file: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Target triple (ex: `x86_64-pc-windows-gnu` para gerar .exe).
        #[arg(long)]
        target: Option<String>,
        #[arg(long, short = 'O', default_value_t = 2)]
        opt_level: u8,
    },
    /// Roda pipeline até MIR sem gerar código. Reporta erros léxicos,
    /// sintáticos, de resolução e de tipo. Use `--emit=mir` para imprimir
    /// o CFG resultante.
    Check {
        file: PathBuf,
        /// Imprime IR intermediária em vez do status. Único valor aceito hoje: `mir`.
        #[arg(long)]
        emit: Option<String>,
    },
    /// Formata um arquivo .vex in-place.
    Fmt { file: PathBuf },
    /// REPL interativo.
    Repl,
    /// Cria scaffold de um novo projeto Vex.
    New { name: String },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Run { file, opt_level } => {
            let output = file.with_extension("");
            let res = compile(CompileRequest {
                source_path: file,
                output_path: output.clone(),
                target: None,
                opt_level,
                check_only: false,
                emit: None,
            });
            if res.is_ok() {
                // Executa o binário recém-gerado.
                let status = std::process::Command::new(&output).status();
                let _ = std::fs::remove_file(&output);
                match status {
                    Ok(s) if s.success() => return ExitCode::SUCCESS,
                    Ok(s)  => return ExitCode::from(s.code().unwrap_or(1) as u8),
                    Err(e) => {
                        eprintln!("erro ao executar binário: {e}");
                        return ExitCode::FAILURE;
                    }
                }
            }
            res
        }
        Commands::Build { file, output, target, opt_level } => {
            let output = output.unwrap_or_else(|| file.with_extension(""));
            compile(CompileRequest {
                source_path: file,
                output_path: output,
                target,
                opt_level,
                check_only: false,
                emit: None,
            })
        }
        Commands::Check { file, emit } => {
            let emit = match emit.as_deref() {
                None => None,
                Some("mir") => Some(EmitKind::Mir),
                Some("liveness") => Some(EmitKind::Liveness),
                Some(other) => {
                    eprintln!("--emit desconhecido: `{other}` (aceita: mir, liveness)");
                    return ExitCode::FAILURE;
                }
            };
            compile(CompileRequest {
                source_path: file.clone(),
                output_path: file,
                target: None,
                opt_level: 0,
                check_only: true,
                emit,
            })
        }
        Commands::Fmt { file } => match std::fs::read_to_string(&file) {
            Ok(src) => {
                let out = vex_fmt::format(&src);
                std::fs::write(&file, out).map_err(DriverError::from)
            }
            Err(e) => Err(DriverError::Io(e)),
        },
        Commands::Repl => {
            eprintln!("repl ainda em construção (Fase 9)");
            Ok(())
        }
        Commands::New { name } => {
            match scaffold_project(&name) {
                Ok(()) => {
                    eprintln!("✓ projeto `{name}` criado");
                    return ExitCode::SUCCESS;
                }
                Err(e) => {
                    eprintln!("erro ao criar projeto: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(DriverError::Io(e)) => {
            eprintln!("erro de I/O: {e}");
            ExitCode::FAILURE
        }
        // Erros estruturados (parse/typeck/codegen) já foram renderizados
        // pelo driver via miette. Apenas saímos com código não-zero.
        Err(_) => ExitCode::FAILURE,
    }
}

/// Cria scaffold de projeto Vex em `<name>/`:
/// - `Vex.toml` (manifesto — formato a evoluir na Fase 8)
/// - `src/main.vex` (hello world)
/// - `.gitignore` (binários gerados)
fn scaffold_project(name: &str) -> std::io::Result<()> {
    let root = std::path::PathBuf::from(name);
    if root.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("diretório `{}` já existe", root.display()),
        ));
    }
    std::fs::create_dir(&root)?;
    std::fs::create_dir(root.join("src"))?;

    std::fs::write(
        root.join("Vex.toml"),
        format!(
            "[project]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"v0.1\"\n",
        ),
    )?;
    std::fs::write(
        root.join("src/main.vex"),
        "fn main() -> void {\n    println(\"Hello from Vex!\")\n}\n",
    )?;
    std::fs::write(
        root.join(".gitignore"),
        "/out/\n*.o\n*.exe\n*.ll\n",
    )?;
    Ok(())
}
