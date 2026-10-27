//! proc - Semantic Process Management CLI
//!
//! A semantic command-line tool for process management.

use clap::{CommandFactory, Parser};
use clap_complete::generate;
use colored::Colorize;
use proc_cli::cli::{Cli, Commands};
use proc_cli::core::Process;
use proc_cli::error::{ExitCode, ProcError};
use std::io;
use std::process;

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
                print_json_error(&error, exit_code, None);
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
            let processes = match &e {
                ProcError::ConfirmationRequired { processes, .. } => Some(processes.as_slice()),
                _ => None,
            };
            print_json_error(&e.to_string(), exit_code as i32, processes);
        } else {
            eprintln!("{}", e);
        }

        process::exit(exit_code as i32);
    }
}

/// Print an error as the standard JSON error object on stdout.
///
/// `confirmation` is `Some(processes)` only for a missing `--yes`: the object
/// then also says `needs_confirmation` and lists what would have been changed.
fn print_json_error(error: &str, exit_code: i32, confirmation: Option<&[Process]>) {
    let mut error_json = serde_json::json!({
        "action": action_name(),
        "success": false,
        "error": error,
        "exit_code": exit_code
    });
    if let Some(processes) = confirmation {
        error_json["needs_confirmation"] = serde_json::json!(true);
        error_json["processes"] = serde_json::json!(processes);
    }
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
