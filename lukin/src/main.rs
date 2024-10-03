use clap::Parser;
use miette::{IntoDiagnostic, Result, WrapErr};
use poki::Poki;

use std::fs::File;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone, Hash, PartialEq, Eq)]
#[command(version, about, long_about)]
struct Args {
    source_path: PathBuf,
    // TODO: Add some flags to make it possible to granularize what lukin displays.
}

fn main() -> Result<()> {
    let args = Args::parse();

    let source = File::open(&args.source_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("unable to read source from {}", args.source_path.display()))?;

    let poki = Poki::deserialize(&source)
        .into_diagnostic()
        .wrap_err("unable to deserialize provided poki file")?;

    println!("{:?}", poki);

    Ok(())
}
