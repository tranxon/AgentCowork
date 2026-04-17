//! rollball-gateway CLI entry point

use clap::Parser;
use rollball_gateway::cli::Cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cli.run()
}
