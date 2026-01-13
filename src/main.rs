//! proc - Semantic Process Management CLI
//!
//! A semantic command-line tool for process management.

use clap::{Parser, Subcommand};
use proc_cli::commands::{
    InfoCommand, KillCommand, OnCommand, PortsCommand, PsCommand, StopCommand, StuckCommand,
    TreeCommand, UnstickCommand,
};
use proc_cli::error::ExitCode;
use std::process;

/// Semantic process management CLI
#[derive(Parser)]
#[command(name = "proc")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
#[command(after_help = "Examples:
  proc on :3000           What's on port 3000?
  proc on 1234            What ports is PID 1234 using?
  proc ps                 List all processes
  proc ps node            Filter by name
  proc ps --cwd .         Processes in current directory
  proc info :3000         Info for process on port 3000
  proc tree node          Process tree for node
  proc kill :3000         Kill process on port 3000
  proc stop node          Stop Node.js gracefully
  proc stuck              Find hung processes
  proc unstick            Attempt to recover stuck processes

Targets: Most commands accept :port, PID, or process name.

For more information, visit: https://github.com/yazeed/proc")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List processes
    #[command(visible_alias = "l")]
    Ps(PsCommand),

    /// Show detailed process information
    #[command(visible_alias = "i")]
    Info(InfoCommand),

    /// Port/process lookup (bidirectional)
    #[command(visible_alias = ":")]
    On(OnCommand),

    /// List all listening ports
    #[command(visible_alias = "p")]
    Ports(PortsCommand),

    /// Kill process(es) forcefully
    #[command(visible_alias = "k")]
    Kill(KillCommand),

    /// Stop process(es) gracefully
    #[command(visible_alias = "s")]
    Stop(StopCommand),

    /// Find stuck/hung processes
    #[command(visible_alias = "x")]
    Stuck(StuckCommand),

    /// Show process tree
    #[command(visible_alias = "t")]
    Tree(TreeCommand),

    /// Attempt to recover stuck processes
    #[command(visible_alias = "u")]
    Unstick(UnstickCommand),
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Ps(cmd) => cmd.execute(),
        Commands::Info(cmd) => cmd.execute(),
        Commands::On(cmd) => cmd.execute(),
        Commands::Ports(cmd) => cmd.execute(),
        Commands::Kill(cmd) => cmd.execute(),
        Commands::Stop(cmd) => cmd.execute(),
        Commands::Stuck(cmd) => cmd.execute(),
        Commands::Tree(cmd) => cmd.execute(),
        Commands::Unstick(cmd) => cmd.execute(),
    };

    if let Err(e) = result {
        eprintln!("{}", e);
        let exit_code = ExitCode::from(&e);
        process::exit(exit_code as i32);
    }
}
