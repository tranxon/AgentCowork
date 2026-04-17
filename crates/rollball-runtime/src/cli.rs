//! CLI definitions

use clap::Parser;

/// Agent Runtime CLI
#[derive(Parser)]
#[command(name = "rollball-runtime")]
#[command(about = "Agent Runtime - unified execution engine")]
pub struct Cli {
    /// Agent ID
    #[arg(long)]
    pub agent_id: String,

    /// Path to manifest.toml
    #[arg(long)]
    pub manifest_path: String,

    /// Working directory
    #[arg(long)]
    pub work_dir: String,

    /// Gateway socket path
    #[arg(long)]
    pub gateway_socket: String,

    /// Enable developer mode
    #[arg(long, default_value = "false")]
    pub dev_mode: bool,

    /// Log level
    #[arg(long, default_value = "info")]
    pub log_level: String,
}

impl Cli {
    /// Run the CLI
    pub fn run(self) -> anyhow::Result<()> {
        // TODO: Initialize runtime and start agent loop
        println!("Starting agent runtime for: {}", self.agent_id);
        unimplemented!()
    }
}
