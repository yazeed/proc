//! proc - Semantic Process Management CLI
//!
//! A semantic command-line tool for process management.

use clap::{Parser, Subcommand};
use proc_cli::commands::{FindCommand, KillCommand, OnCommand, PortsCommand, StuckCommand};
use proc_cli::error::ExitCode;
use std::process;

/// Semantic process management CLI
#[derive(Parser)]
#[command(name = "proc")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
#[command(after_help = "Examples:
  proc find node          Find all Node.js processes
  proc on :3000           What's on port 3000?
  proc ports              List all listening ports
  proc kill node          Kill all Node.js processes
  proc stuck              Find hung processes

For more information, visit: https://github.com/yazeed/proc")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Find processes by name
    #[command(visible_alias = "f")]
    Find(FindCommand),

    /// Show what process is on a port
    #[command(visible_alias = ":")]
    On(OnCommand),

    /// List all listening ports
    #[command(visible_alias = "p")]
    Ports(PortsCommand),

    /// Kill process(es)
    #[command(visible_alias = "k")]
    Kill(KillCommand),

    /// Find stuck/hung processes
    #[command(visible_alias = "x")]
    Stuck(StuckCommand),
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Find(cmd) => cmd.execute(),
        Commands::On(cmd) => cmd.execute(),
        Commands::Ports(cmd) => cmd.execute(),
        Commands::Kill(cmd) => cmd.execute(),
        Commands::Stuck(cmd) => cmd.execute(),
    };

    if let Err(e) = result {
        eprintln!("{}", e);
        let exit_code = ExitCode::from(&e);
        process::exit(exit_code as i32);
    }
}
