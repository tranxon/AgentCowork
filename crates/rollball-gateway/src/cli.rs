//! Gateway CLI

use clap::{Parser, Subcommand};

/// Gateway CLI
#[derive(Parser)]
#[command(name = "rollball-gateway")]
#[command(about = "Gateway - Agent lifecycle manager")]
pub struct Cli {
    /// Run as daemon
    #[arg(long)]
    pub daemon: bool,

    /// Subcommands
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Install a .agent package
    Install {
        /// Package path
        package: String,
    },
    /// Uninstall an agent
    Uninstall {
        /// Agent ID
        agent_id: String,
    },
    /// Start an agent
    Start {
        /// Agent ID
        agent_id: String,
    },
    /// Stop an agent
    Stop {
        /// Agent ID
        agent_id: String,
    },
    /// List installed agents
    List,
}

impl Cli {
    /// Run the CLI
    pub fn run(self) -> anyhow::Result<()> {
        match self.command {
            Some(Commands::Install { package }) => {
                println!("Installing package: {}", package);
            }
            Some(Commands::Uninstall { agent_id }) => {
                println!("Uninstalling agent: {}", agent_id);
            }
            Some(Commands::Start { agent_id }) => {
                println!("Starting agent: {}", agent_id);
            }
            Some(Commands::Stop { agent_id }) => {
                println!("Stopping agent: {}", agent_id);
            }
            Some(Commands::List) => {
                println!("Listing installed agents");
            }
            None => {
                if self.daemon {
                    println!("Starting gateway in daemon mode");
                    // TODO: Start daemon
                } else {
                    println!("Gateway CLI - use subcommands");
                }
            }
        }
        Ok(())
    }
}
