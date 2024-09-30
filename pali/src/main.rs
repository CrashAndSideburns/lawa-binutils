mod assemble;
mod lex;
mod parse;

use clap::Parser;
use miette::{IntoDiagnostic, Result, WrapErr};

use std::fs::{read_to_string, File};
use std::path::PathBuf;

use assemble::Assembler;

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

    let mut output_file = File::create(&output_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("unable to write output to {}", output_path.display()))?;

    Assembler::try_new(&source)?
        .assemble()?
        .serialize(&mut output_file)
        .into_diagnostic()
        .wrap_err("unable to serialize assembled poki file")
}
