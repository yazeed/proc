//! proc - Semantic Process Management CLI
//!
//! A semantic command-line tool for process management.

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use proc_cli::commands::{
    ByCommand, ForCommand, InCommand, InfoCommand, KillCommand, ListCommand, OnCommand,
    PortsCommand, StopCommand, StuckCommand, TreeCommand, UnstickCommand,
};
use proc_cli::error::ExitCode;
use std::io;
use std::process;

const VERSION_INFO: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "\nhttps://github.com/yazeed/proc",
    "\nLicense: MIT"
);

/// Semantic CLI tool for process management. Target by port, PID, name or path.
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

  Find by File:
    proc for ./script.py           What's running this file?
    proc for /usr/bin/node         Processes running this executable
    proc for app.log               What has this file open?

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
    proc kill :3000,node           Kill port 3000 and node processes
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

/// Extract the unknown argument from a clap error message
fn extract_unknown_arg(msg: &str) -> Option<String> {
    // Pattern: "unexpected argument 'X' found"
    let start = msg.find("unexpected argument '")? + "unexpected argument '".len();
    let end = msg[start..].find('\'')?;
    Some(msg[start..start + end].to_string())
}

/// Simple edit distance for flag suggestion
fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (m, n) = (a.len(), b.len());
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for (i, row) in dp.iter_mut().enumerate().take(m + 1) {
        row[0] = i;
    }
    for (j, val) in dp[0].iter_mut().enumerate().take(n + 1) {
        *val = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[m][n]
}

/// Find the closest matching flag for a subcommand
fn suggest_flag(unknown: &str, subcmd_name: &str) -> Option<String> {
    let cmd = Cli::command();
    let subcmd = cmd.find_subcommand(subcmd_name)?;
    let unknown_clean = unknown.trim_start_matches('-');

    let mut best: Option<String> = None;
    let mut best_dist = usize::MAX;

    for arg in subcmd.get_arguments() {
        if let Some(long) = arg.get_long() {
            // Check edit distance (skip very short flags and large length differences)
            let len_diff = (unknown_clean.len() as isize - long.len() as isize).unsigned_abs();
            if len_diff <= 2 && long.len() >= 3 {
                let dist = edit_distance(unknown_clean, long);
                if dist < best_dist {
                    best_dist = dist;
                    best = Some(format!("--{}", long));
                }
            }

            // Also check if unknown is a substring of the flag (e.g., "cpu" → "min-cpu")
            if long.contains(unknown_clean) && unknown_clean.len() >= 3 {
                return Some(format!("--{}", long));
            }
        }
    }

    // Only suggest if edit distance is reasonable
    let threshold = if unknown_clean.len() <= 4 { 2 } else { 3 };
    if best_dist <= threshold {
        best
    } else {
        None
    }
}

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            use clap::error::ErrorKind;

            // Let help/version pass through as-is
            if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
                e.exit();
            }

            // Strip clap's misleading "tip:" lines
            let msg = e.to_string();
            let cleaned: String = msg
                .lines()
                .filter(|line| !line.trim_start().starts_with("tip:"))
                .collect::<Vec<_>>()
                .join("\n");
            eprint!("{}", cleaned);

            // Try to suggest a similar flag
            if matches!(
                e.kind(),
                ErrorKind::UnknownArgument | ErrorKind::InvalidValue
            ) {
                let args: Vec<String> = std::env::args().collect();
                let subcmd = args.iter().skip(1).find(|a| !a.starts_with('-'));
                let unknown = extract_unknown_arg(&msg);
                if let (Some(subcmd), Some(unknown)) = (subcmd, unknown) {
                    if let Some(suggestion) = suggest_flag(&unknown, subcmd) {
                        eprintln!("\n  tip: did you mean '{}'?", suggestion);
                    }
                }
            }

            process::exit(2);
        }
    };

    let result = match cli.command {
        Commands::On(cmd) => cmd.execute(),
        Commands::For(cmd) => cmd.execute(),
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
        Commands::Completions { shell } => {
            generate(shell, &mut Cli::command(), "proc", &mut io::stdout());
            Ok(())
        }
        Commands::Manpage => {
            let cmd = Cli::command();
            let man = clap_mangen::Man::new(cmd);
            man.render(&mut io::stdout())
                .expect("Failed to generate man page");
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("{}", e);
        let exit_code = ExitCode::from(&e);
        process::exit(exit_code as i32);
    }
}
