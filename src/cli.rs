//! The command-line definition: `Cli` and its subcommands.
//!
//! Lives in the library so tests can read the clap definitions
//! (`Cli::command()`) and check them against what they exercise.

use crate::commands::{
    ByCommand, ForCommand, FreeCommand, FreezeCommand, InCommand, InfoCommand, KillCommand,
    ListCommand, OnCommand, OrphansCommand, PortsCommand, StopCommand, StuckCommand, ThawCommand,
    TreeCommand, UnstickCommand, WaitCommand, WatchCommand, WhyCommand,
};
use clap::{Parser, Subcommand};
use clap_complete::Shell;

const VERSION_INFO: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "\nhttps://github.com/yazeed/proc-cli",
    "\nLicense: MIT"
);

/// Semantic CLI tool for process management. Target by port, PID, name or path.
#[derive(Parser)]
#[command(name = "proc")]
#[command(author, version = VERSION_INFO, about, long_about = None)]
#[command(propagate_version = true)]
#[command(
    after_help = "Targets: :port, PID, or process name. Give several as separate arguments.
Run 'proc --help' for examples or visit https://github.com/yazeed/proc-cli"
)]
#[command(after_long_help = "EXAMPLES:

  Port/Process Lookup:
    proc on :3000                  What's on port 3000?
    proc on :3000 :8080            What's on several ports?
    proc on node                   What ports are the processes named node using?

  Find by File:
    proc for ./script.py           What's running this file?
    proc for /usr/bin/node         Processes running this executable
    proc for app.log               What has this file open?

  Filter by Name:
    proc by node                   Processes named 'node' (the whole name)
    proc by node --cmd             Anything with 'node' in its name or command line
    proc by node --in .            Node processes in current directory
    proc by node --min-cpu 5       Node processes using >5% CPU

  Filter by Directory:
    proc in .                      Processes in current directory
    proc in . --by node            Node processes in cwd

  List All:
    proc list                      All processes
    proc list --min-cpu 10         Processes using >10% CPU

  Info/Kill/Stop (multi-target):
    proc info :3000 :8080          Info for several targets
    proc kill :3000 node           Kill port 3000 and the processes named node
    proc kill serve --cmd          Kill processes with 'serve' in their command line
    proc kill 1234 5678 :8080      Kill two PIDs and whatever is on 8080
    proc stop :3000 :8080          Stop several targets gracefully

  Watch (Real-Time):
    proc watch                     Watch all processes (alias: proc top)
    proc watch node                Watch the processes named node
    proc watch :3000               Watch process on port 3000
    proc watch :3000 :8080 node    Watch several targets
    proc watch -n 1 --sort mem     1s refresh, sorted by memory
    proc watch --in . --by node    Node processes in current directory

  Freeze/Thaw:
    proc freeze node               Pause the processes named node (SIGSTOP)
    proc freeze :3000 --yes        Freeze by port, skip confirm
    proc thaw node                 Resume frozen processes (SIGCONT)

  Free Ports:
    proc free :3000                Kill process, verify port freed
    proc free :3000 :8080 --yes    Free several ports

  Diagnostics:
    proc why :3000                 Why is port 3000 busy? (ancestry)
    proc orphans                   Find orphaned processes
    proc orphans --kill            Find and kill orphans

  Wait:
    proc wait node                 Wait until node processes exit
    proc wait 1234 5678            Wait until both exit
    proc wait :3000 --timeout 60   Wait up to 60s for port to free
    proc wait node -n 10 -q        Check every 10s, quiet mode

  Other:
    proc ports                     List all listening ports
    proc tree --min-cpu 5          Process tree filtered by CPU
    proc stuck                     Find hung processes
    proc unstick --force           Recover or terminate stuck processes
    proc stop nginx --signal HUP   Send custom signal (config reload)

Targets: :port, PID, or process name. Give several as separate arguments.
A name matches the whole process name, ignoring case, like killall and pkill:
'node' does not match 'Claude Helper'. Add --cmd to match any part of the name
or the full command line instead (like pkill -f). A path is not a name: use
--cmd, or 'proc for <file>' for an executable. proc doesn't expand wildcards;
an unquoted node* is expanded by your shell into file names; use --cmd for
partial matches.
Comma-separated targets (:3000,:8080) still work but are deprecated and will be
removed in a future major release.
--in takes the next word as its directory unless that word starts with '-':
put it after the targets, or write --in=DIR.
For more information, visit: https://github.com/yazeed/proc-cli")]
pub struct Cli {
    /// The subcommand; none prints the orientation text
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Every `proc` subcommand.
#[derive(Subcommand)]
pub enum Commands {
    /// Port/process lookup (bidirectional)
    #[command(visible_alias = ":")]
    On(OnCommand),

    /// Find processes by file path
    #[command(visible_alias = "f")]
    For(ForCommand),

    /// Filter processes by name
    #[command(visible_alias = "b")]
    By(ByCommand),

    /// Filter processes by working directory
    In(InCommand),

    /// List processes
    #[command(visible_aliases = ["l", "ps"])]
    List(ListCommand),

    /// Show detailed process information
    #[command(visible_aliases = ["i", "inspect"])]
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

    /// Watch processes in real-time
    #[command(visible_aliases = ["w", "top"])]
    Watch(WatchCommand),

    /// Find stuck/hung processes
    #[command(visible_alias = "x")]
    Stuck(StuckCommand),

    /// Attempt to recover stuck processes
    #[command(visible_alias = "u")]
    Unstick(UnstickCommand),

    /// Freeze (pause) process(es) with SIGSTOP
    Freeze(FreezeCommand),

    /// Resume frozen process(es) with SIGCONT
    Thaw(ThawCommand),

    /// Find orphaned processes
    #[command(visible_alias = "o")]
    Orphans(OrphansCommand),

    /// Trace why a port is busy or show process ancestry
    Why(WhyCommand),

    /// Free port(s) by killing process and verifying availability
    Free(FreeCommand),

    /// Wait for process(es) to exit
    Wait(WaitCommand),

    /// Generate shell completions
    #[command(hide = true)]
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Generate man page
    #[command(hide = true)]
    Manpage,
}
