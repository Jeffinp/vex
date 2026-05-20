//! `vex` — CLI principal da linguagem Vex.

use std::path::PathBuf;
use clap::{Parser, Subcommand};
use vex_driver::{CompileRequest, compile};

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
    /// Apenas faz type-check, sem gerar código.
    Check { file: PathBuf },
    /// Formata um arquivo .vex in-place.
    Fmt { file: PathBuf },
    /// REPL interativo.
    Repl,
    /// Cria scaffold de um novo projeto Vex.
    New { name: String },
}

fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run { file, opt_level } => {
            let output = file.with_extension("");
            compile(CompileRequest {
                source_path: file,
                output_path: output.clone(),
                target: None,
                opt_level,
                check_only: false,
            }).map_err(|e| miette::miette!("{e}"))?;
            // TODO: spawn binário gerado
            eprintln!("(run) compilation pipeline ainda em construção");
        }
        Commands::Build { file, output, target, opt_level } => {
            let output = output.unwrap_or_else(|| file.with_extension(""));
            compile(CompileRequest {
                source_path: file,
                output_path: output,
                target,
                opt_level,
                check_only: false,
            }).map_err(|e| miette::miette!("{e}"))?;
        }
        Commands::Check { file } => {
            compile(CompileRequest {
                source_path: file.clone(),
                output_path: file,
                target: None,
                opt_level: 0,
                check_only: true,
            }).map_err(|e| miette::miette!("{e}"))?;
        }
        Commands::Fmt { file } => {
            let src = std::fs::read_to_string(&file).map_err(|e| miette::miette!("{e}"))?;
            let out = vex_fmt::format(&src);
            std::fs::write(&file, out).map_err(|e| miette::miette!("{e}"))?;
        }
        Commands::Repl => eprintln!("repl ainda em construção (Fase 5)"),
        Commands::New { name } => eprintln!("scaffold do projeto `{name}` ainda em construção"),
    }
    Ok(())
}
