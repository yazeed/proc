//! proc - Semantic Process Management CLI
//!
//! A semantic command-line tool for process management.

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use colored::Colorize;
use proc_cli::commands::{
    ByCommand, ForCommand, FreeCommand, FreezeCommand, InCommand, InfoCommand, KillCommand,
    ListCommand, OnCommand, OrphansCommand, PortsCommand, StopCommand, StuckCommand, ThawCommand,
    TreeCommand, UnstickCommand, WaitCommand, WatchCommand, WhyCommand,
};
use proc_cli::error::{ExitCode, ProcError};
use std::io;
use std::process;

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
    after_help = "Targets: :port, PID, or process name. Comma-separate for multiple.
Run 'proc --help' for examples or visit https://github.com/yazeed/proc-cli"
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

  Watch (Real-Time):
    proc watch                     Watch all processes (alias: proc top)
    proc watch node                Watch node processes
    proc watch :3000               Watch process on port 3000
    proc watch -n 1 --sort mem     1s refresh, sorted by memory
    proc watch --in . --by node    Node processes in current directory

  Freeze/Thaw:
    proc freeze node               Pause node processes (SIGSTOP)
    proc freeze :3000 --yes        Freeze by port, skip confirm
    proc thaw node                 Resume frozen processes (SIGCONT)

  Free Ports:
    proc free :3000                Kill process, verify port freed
    proc free :3000,:8080 --yes    Free multiple ports

  Diagnostics:
    proc why :3000                 Why is port 3000 busy? (ancestry)
    proc orphans                   Find orphaned processes
    proc orphans --kill            Find and kill orphans

  Wait:
    proc wait node                 Wait until node processes exit
    proc wait :3000 --timeout 60   Wait up to 60s for port to free
    proc wait node -n 10 -q        Check every 10s, quiet mode

  Other:
    proc ports                     List all listening ports
    proc tree --min-cpu 5          Process tree filtered by CPU
    proc stuck                     Find hung processes
    proc unstick --force           Recover or terminate stuck processes
    proc stop nginx --signal HUP   Send custom signal (config reload)

Targets: :port, PID, or process name. Comma-separate for multiple.
For more information, visit: https://github.com/yazeed/proc-cli")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

/// What a bare `proc` prints.
///
/// The full command list is a menu, not an orientation: it tells a newcomer
/// what exists without telling them how the tool thinks. The one thing worth
/// learning first is that a target is the same everywhere, so that is what
/// this leads with — and every line here is a command that runs as shown.
fn print_orientation() {
    println!(
        "{} {}",
        "proc".bold(),
        "— semantic process management".bright_black()
    );
    println!();
    println!(
        "  {}",
        "A target is the same everywhere: :port, PID, or name.".bright_black()
    );
    println!();
    for (cmd, what) in [
        ("proc on :3000", "what's on port 3000"),
        ("proc in .", "what's running in this directory"),
        ("proc by node", "processes named node"),
        ("proc kill :3000", "stop whatever holds the port"),
    ] {
        println!(
            "    {} {}",
            format!("{:<20}", cmd).cyan(),
            what.bright_black()
        );
    }
    println!();
    println!(
        "  {} {}",
        format!("{:<8}", "Common:").bright_black(),
        "on · in · by · ports · info · kill · stop · free · why".white()
    );
    println!();
    println!(
        "  {} {}  {}",
        format!("{:<20}", "proc --help").cyan(),
        "all 19 commands".bright_black(),
        "· https://proc-cli.com".bright_black()
    );
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

            // Try to suggest a similar flag
            let mut suggestion = None;
            if matches!(
                e.kind(),
                ErrorKind::UnknownArgument | ErrorKind::InvalidValue
            ) {
                let args: Vec<String> = std::env::args().collect();
                let subcmd = args.iter().skip(1).find(|a| !a.starts_with('-'));
                let unknown = extract_unknown_arg(&msg);
                if let (Some(subcmd), Some(unknown)) = (subcmd, unknown) {
                    suggestion = suggest_flag(&unknown, subcmd);
                }
            }

            // Usage errors are invalid input, same as runtime validation errors
            let exit_code = ExitCode::InvalidInput as i32;

            if wants_json() {
                let mut error = cleaned
                    .trim()
                    .trim_start_matches("error: ")
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .to_string();
                if let Some(ref suggestion) = suggestion {
                    error.push_str(&format!("\n  Try: did you mean '{}'?", suggestion));
                }
                print_json_error(&error, exit_code);
            } else {
                eprint!("{}", cleaned);
                if let Some(suggestion) = suggestion {
                    eprintln!("\n  tip: did you mean '{}'?", suggestion);
                }
            }

            process::exit(exit_code);
        }
    };

    let Some(command) = cli.command else {
        print_orientation();
        return;
    };

    let result = match command {
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
        Commands::Watch(cmd) => cmd.execute(),
        Commands::Stuck(cmd) => cmd.execute(),
        Commands::Unstick(cmd) => cmd.execute(),
        Commands::Freeze(cmd) => cmd.execute(),
        Commands::Thaw(cmd) => cmd.execute(),
        Commands::Orphans(cmd) => cmd.execute(),
        Commands::Why(cmd) => cmd.execute(),
        Commands::Free(cmd) => cmd.execute(),
        Commands::Wait(cmd) => cmd.execute(),
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
        let exit_code = ExitCode::from(&e);

        // The command already printed its result; only the exit code is left
        if matches!(e, ProcError::Reported { .. }) {
            process::exit(exit_code as i32);
        }

        // If --json was requested, output error as JSON to stdout
        if wants_json() {
            print_json_error(&e.to_string(), exit_code as i32);
        } else {
            eprintln!("{}", e);
        }

        process::exit(exit_code as i32);
    }
}

/// Print an error as the standard JSON error object on stdout
fn print_json_error(error: &str, exit_code: i32) {
    let error_json = serde_json::json!({
        "action": action_name(),
        "success": false,
        "error": error,
        "exit_code": exit_code
    });
    println!("{}", serde_json::to_string_pretty(&error_json).unwrap());
}

/// Canonical command name for the JSON `action` field, resolving aliases
/// (e.g. `proc k` → "kill"). Falls back to the raw argument, or "proc".
fn action_name() -> String {
    let first = std::env::args().skip(1).find(|a| !a.starts_with('-'));
    match first {
        Some(arg) => Cli::command()
            .find_subcommand(&arg)
            .map(|cmd| cmd.get_name().to_string())
            .unwrap_or(arg),
        None => "proc".to_string(),
    }
}

/// Check if --json or -j was passed in the command line arguments
fn wants_json() -> bool {
    std::env::args()
        .skip(1)
        .take_while(|a| a != "--")
        .any(|a| is_json_flag(&a))
}

/// Matches `--json`, `-j`, and short-flag clusters such as `-yj`.
///
/// Scanning a cluster stops at the first short flag that takes a value,
/// because everything after it is that flag's value (`-bj` is `--by j`).
fn is_json_flag(arg: &str) -> bool {
    if arg == "--json" {
        return true;
    }
    let Some(cluster) = arg.strip_prefix('-') else {
        return false;
    };
    if cluster.starts_with('-') {
        return false;
    }
    for c in cluster.chars() {
        match c {
            'j' => return true,
            'b' | 'd' | 'i' | 'l' | 'n' | 'p' | 'r' | 's' | 'S' | 't' => return false,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// Commands that change processes. They ask before acting, so they take
    /// --yes/-y and --dry-run (`orphans` and `stuck` only change them with --kill)
    const CHANGES_PROCESSES: &[&str] = &[
        "kill", "stop", "free", "freeze", "thaw", "unstick", "orphans", "stuck",
    ];

    /// Not process commands: they print shell completions or the man page
    const NOT_PROCESS_COMMANDS: &[&str] = &["completions", "manpage"];

    #[test]
    fn every_command_follows_the_flag_conventions() {
        let cli = Cli::command();
        let mut problems = Vec::new();

        for cmd in cli.get_subcommands() {
            let name = cmd.get_name();
            if NOT_PROCESS_COMMANDS.contains(&name) {
                continue;
            }
            let mut need = |long: &str, short: Option<char>| {
                let found = cmd.get_arguments().find(|a| a.get_long() == Some(long));
                match (found, short) {
                    (None, _) => problems.push(format!("{name}: no --{long}")),
                    (Some(a), Some(s)) if a.get_short() != Some(s) => {
                        problems.push(format!("{name}: --{long} should have -{s}"))
                    }
                    _ => {}
                }
            };
            need("json", Some('j'));
            need("verbose", Some('v'));
            if CHANGES_PROCESSES.contains(&name) {
                need("yes", Some('y'));
                need("dry-run", None);
            }
        }

        for name in CHANGES_PROCESSES.iter().chain(NOT_PROCESS_COMMANDS) {
            if cli.find_subcommand(name).is_none() {
                problems.push(format!("{name} is listed here but isn't a command"));
            }
        }
        assert!(
            problems.is_empty(),
            "convention broken:\n{}",
            problems.join("\n")
        );
    }
}
