//! rollball-runtime CLI entry point

use clap::Parser;
use rollball_runtime::cli::Cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cli.run()
}
