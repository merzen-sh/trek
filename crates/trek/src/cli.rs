use crate::server::run_server;
use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "trek", version, about)]
struct Cli {
    #[arg(short, long, default_value = "8080")]
    port: u16,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Runs the web server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Serve { port }) => {
            run_server(*port)?;
        }
        None => {
            run_server(cli.port)?;
        }
    }

    Ok(())
}
