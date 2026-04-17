//! rollball-sign CLI

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rollball-sign")]
#[command(about = "Sign .agent packages")]
struct Cli {
    /// Input .agent package path
    #[arg(short, long)]
    input: PathBuf,

    /// Key file path
    #[arg(short, long)]
    key: PathBuf,

    /// Output signed package path
    #[arg(short, long)]
    output: PathBuf,
}

fn main() {
    let _cli = Cli::parse();
    // TODO: Implement signing CLI
    println!("rollball-sign: TODO - implement package signing");
}
