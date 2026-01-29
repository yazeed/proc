//! proc - Semantic Process Management CLI
//!
//! A semantic command-line tool for process management.

use clap::{Parser, Subcommand};
use proc_cli::commands::{
    ByCommand, InCommand, InfoCommand, KillCommand, ListCommand, OnCommand, PortsCommand,
    StopCommand, StuckCommand, TreeCommand, UnstickCommand,
};
use proc_cli::error::ExitCode;
use std::process;

const VERSION_INFO: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "\nhttps://github.com/yazeed/proc",
    "\nLicense: MIT"
);

/// Semantic process management CLI
#[derive(Parser)]
#[command(name = "proc")]
#[command(author, version = VERSION_INFO, about, long_about = None)]
#[command(propagate_version = true)]
#[command(
    after_help = "Targets: :port, PID, or process name. Comma-separate for multiple.
Run 'proc --help' for examples or visit https://github.com/yazeed/proc"
)]
#[command(after_long_help = "EXAMPLES:

  Port/Process Lookup:
    proc on :3000                  What's on port 3000?
    proc on :3000,:8080            What's on multiple ports?
    proc on node                   What ports are node processes using?

  Filter by Name:
    proc by node                   Processes named 'node'
    proc by node --in .            Node processes in current directory
    proc by node --min-cpu 5       Node processes using >5% CPU

  Filter by Directory:
    proc in .                      Processes in current directory
    proc in . --by node            Node processes in cwd

  List All:
    proc list                      All processes
    proc list --min-cpu 10         Processes using >10% CPU

  Info/Kill/Stop (multi-target):
    proc info :3000,:8080          Info for multiple targets
    proc kill :3000,node -y        Kill port 3000 and node processes
    proc stop :3000,:8080          Stop multiple targets gracefully

  Other:
    proc ports                     List all listening ports
    proc tree --min-cpu 5          Process tree filtered by CPU
    proc stuck                     Find hung processes
    proc unstick --force           Recover or terminate stuck processes

Targets: :port, PID, or process name. Comma-separate for multiple.
For more information, visit: https://github.com/yazeed/proc")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Port/process lookup (bidirectional)
    #[command(visible_alias = ":")]
    On(OnCommand),

    /// Filter processes by name
    #[command(visible_alias = "b")]
    By(ByCommand),

    /// Filter processes by working directory
    In(InCommand),

    /// List processes
    #[command(visible_aliases = ["l", "ps"])]
    List(ListCommand),

    /// Show detailed process information
    #[command(visible_alias = "i")]
    Info(InfoCommand),

    /// List all listening ports
    #[command(visible_alias = "p")]
    Ports(PortsCommand),

    /// Kill process(es) forcefully
    #[command(visible_alias = "k")]
    Kill(KillCommand),

    /// Stop process(es) gracefully
    #[command(visible_alias = "s")]
    Stop(StopCommand),

    /// Show process tree
    #[command(visible_alias = "t")]
    Tree(TreeCommand),

    /// Find stuck/hung processes
    #[command(visible_alias = "x")]
    Stuck(StuckCommand),

    /// Attempt to recover stuck processes
    #[command(visible_alias = "u")]
    Unstick(UnstickCommand),
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::On(cmd) => cmd.execute(),
        Commands::By(cmd) => cmd.execute(),
        Commands::In(cmd) => cmd.execute(),
        Commands::List(cmd) => cmd.execute(),
        Commands::Info(cmd) => cmd.execute(),
        Commands::Ports(cmd) => cmd.execute(),
        Commands::Kill(cmd) => cmd.execute(),
        Commands::Stop(cmd) => cmd.execute(),
        Commands::Tree(cmd) => cmd.execute(),
        Commands::Stuck(cmd) => cmd.execute(),
        Commands::Unstick(cmd) => cmd.execute(),
    };

    if let Err(e) = result {
        eprintln!("{}", e);
        let exit_code = ExitCode::from(&e);
        process::exit(exit_code as i32);
    }
}
