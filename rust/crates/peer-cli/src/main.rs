mod commands;
mod output;

use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};
use commands::{dial::DialArgs, listen::ListenArgs};

#[derive(Debug, Parser)]
#[command(name = "peer-cli", author, version, about = "Peer-to-peer ping/pong tester for BakopaVR")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Start a listener that prints its multi-address and answers ping requests with pong.
    Listen(ListenArgs),
    /// Dial a listener multi-address and perform a ping/pong round-trip measurement.
    Dial(DialArgs),
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    if let Err(err) = run().await {
        eprintln!("error: {err:?}");
        return ExitCode::from(1);
    }
    ExitCode::from(0)
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Listen(args) => commands::listen::run(args).await?,
        Command::Dial(args) => commands::dial::run(args).await?,
    }
    Ok(())
}
