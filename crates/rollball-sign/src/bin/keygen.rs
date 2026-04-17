//! rollball-keygen CLI

use clap::Parser;

#[derive(Parser)]
#[command(name = "rollball-keygen")]
#[command(about = "Generate Ed25519 key pairs for .agent package signing")]
struct Cli {
    /// Key type (developer or platform)
    #[arg(short, long, default_value = "developer")]
    r#type: String,

    /// Output directory for key files
    #[arg(short, long)]
    output_dir: Option<String>,
}

fn main() {
    let _cli = Cli::parse();
    // TODO: Implement key generation CLI
    println!("rollball-keygen: TODO - implement key generation");
}
