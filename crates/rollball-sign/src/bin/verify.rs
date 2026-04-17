//! rollball-verify CLI

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rollball-verify")]
#[command(about = "Verify .agent package signatures")]
struct Cli {
    /// .agent package path to verify
    package: PathBuf,
}

fn main() {
    let _cli = Cli::parse();
    // TODO: Implement verification CLI
    println!("rollball-verify: TODO - implement package verification");
}
