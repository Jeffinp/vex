//! `vex` — CLI principal da linguagem Vex.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use vex_driver::{CompileRequest, DriverError, compile};

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
    /// Apenas faz lex + parse (e futuramente type-check), sem gerar código.
    Check { file: PathBuf },
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
            compile(CompileRequest {
                source_path: file,
                output_path: output,
                target: None,
                opt_level,
                check_only: false,
            })
        }
        Commands::Build { file, output, target, opt_level } => {
            let output = output.unwrap_or_else(|| file.with_extension(""));
            compile(CompileRequest {
                source_path: file,
                output_path: output,
                target,
                opt_level,
                check_only: false,
            })
        }
        Commands::Check { file } => compile(CompileRequest {
            source_path: file.clone(),
            output_path: file,
            target: None,
            opt_level: 0,
            check_only: true,
        }),
        Commands::Fmt { file } => match std::fs::read_to_string(&file) {
            Ok(src) => {
                let out = vex_fmt::format(&src);
                std::fs::write(&file, out).map_err(DriverError::from)
            }
            Err(e) => Err(DriverError::Io(e)),
        },
        Commands::Repl => {
            eprintln!("repl ainda em construção (Fase 7)");
            Ok(())
        }
        Commands::New { name } => {
            eprintln!("scaffold do projeto `{name}` ainda em construção (Fase 7)");
            Ok(())
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
