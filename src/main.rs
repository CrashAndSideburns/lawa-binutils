mod lex;
mod parse;
mod poki;

use clap::Parser;
use miette::{IntoDiagnostic, Result, WrapErr};

use std::fs::{read_to_string, write};
use std::path::PathBuf;

#[derive(Parser, Debug, Clone, Hash, PartialEq, Eq)]
#[command(version, about, long_about)]
struct Args {
    source_path: PathBuf,
    output_path: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let source_path = args.source_path;
    let output_path = args
        .output_path
        .unwrap_or_else(|| source_path.with_extension("poki"));

    let source = read_to_string(&source_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("unable to read source from {}", source_path.display()))?;

    let parser = parse::Parser::new(&source);

    let program = parser.parse()?;
    let poki = poki::Poki::from_program(program)?.to_bytes();

    write(&output_path, poki)
        .into_diagnostic()
        .wrap_err_with(|| format!("unable to write output to {}", output_path.display()))
}
