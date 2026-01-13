//! Unstick command - Attempt to recover stuck processes
//!
//! Tries gentle recovery signals. Only terminates with --force.
//!
//! Recovery sequence:
//! 1. SIGCONT (wake if stopped)
//! 2. SIGINT (interrupt, like Ctrl+C)
//!
//! With --force:
//! 3. SIGTERM (polite termination request)
//! 4. SIGKILL (force, last resort)
//!
//! Usage:
//!   proc unstick           # Find and unstick all stuck processes
//!   proc unstick :3000     # Unstick process on port 3000
//!   proc unstick 1234      # Unstick PID 1234
//!   proc unstick node      # Unstick stuck node processes

use crate::core::{resolve_target, Process};
use crate::error::{ProcError, Result};
use crate::ui::{OutputFormat, Printer};
use clap::Args;
use colored::*;
use dialoguer::Confirm;
use serde::Serialize;
use std::time::Duration;

#[cfg(unix)]
use nix::sys::signal::{kill, Signal};
#[cfg(unix)]
use nix::unistd::Pid;

/// Attempt to recover stuck processes
#[derive(Args, Debug)]
pub struct UnstickCommand {
    /// Target: PID, :port, or name (optional - finds all stuck if omitted)
    target: Option<String>,

    /// Minimum seconds of high CPU before considered stuck (for auto-discovery)
    #[arg(long, short, default_value = "300")]
    timeout: u64,

    /// Force termination if recovery fails
    #[arg(long, short = 'f')]
    force: bool,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    yes: bool,

    /// Show what would be done without doing it
    #[arg(long)]
    dry_run: bool,

    /// Output as JSON
    #[arg(long, short)]
    json: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum Outcome {
    Recovered,  // Process unstuck and still running
    Terminated, // Had to kill it (only with --force)
    StillStuck, // Could not recover, not terminated (no --force)
    NotStuck,   // Process wasn't stuck to begin with
    Failed(String),
}

impl UnstickCommand {
    pub fn execute(&self) -> Result<()> {
        let format = if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };
        let printer = Printer::new(format, false);

        // Get processes to unstick
        let stuck = if let Some(ref target) = self.target {
            // Specific target
            self.resolve_target_processes(target)?
        } else {
            // Auto-discover stuck processes
            let timeout = Duration::from_secs(self.timeout);
            Process::find_stuck(timeout)?
        };

        if stuck.is_empty() {
            if self.json {
                printer.print_json(&UnstickOutput {
                    action: "unstick",
                    success: true,
                    dry_run: self.dry_run,
                    force: self.force,
                    found: 0,
                    recovered: 0,
                    not_stuck: 0,
                    still_stuck: 0,
                    terminated: 0,
                    failed: 0,
                    processes: Vec::new(),
                });
            } else if self.target.is_some() {
                printer.warning("Target process not found");
            } else {
                printer.success("No stuck processes found");
            }
            return Ok(());
        }

        // Show stuck processes
        if !self.json {
            self.show_processes(&stuck);
        }

        // Dry run
        if self.dry_run {
            if self.json {
                printer.print_json(&UnstickOutput {
                    action: "unstick",
                    success: true,
                    dry_run: true,
                    force: self.force,
                    found: stuck.len(),
                    recovered: 0,
                    not_stuck: 0,
                    still_stuck: 0,
                    terminated: 0,
                    failed: 0,
                    processes: stuck
                        .iter()
                        .map(|p| ProcessOutcome {
                            pid: p.pid,
                            name: p.name.clone(),
                            outcome: "would_attempt".to_string(),
                        })
                        .collect(),
                });
            } else {
                println!(
                    "\n{} Dry run: Would attempt to unstick {} process{}",
                    "ℹ".blue().bold(),
                    stuck.len().to_string().cyan().bold(),
                    if stuck.len() == 1 { "" } else { "es" }
                );
                if self.force {
                    println!("  With --force: will terminate if recovery fails");
                } else {
                    println!("  Without --force: will only attempt recovery");
                }
                println!();
            }
            return Ok(());
        }

        // Confirm
        if !self.yes && !self.json {
            if self.force {
                println!(
                    "\n{} With --force: processes will be terminated if recovery fails.\n",
                    "!".yellow().bold()
                );
            } else {
                println!(
                    "\n{} Will attempt recovery only. Use --force to terminate if needed.\n",
                    "ℹ".blue().bold()
                );
            }

            let prompt = format!(
                "Unstick {} process{}?",
                stuck.len(),
                if stuck.len() == 1 { "" } else { "es" }
            );

            if !Confirm::new()
                .with_prompt(prompt)
                .default(false)
                .interact()?
            {
                printer.warning("Aborted");
                return Ok(());
            }
        }

        // Attempt to unstick each process
        let mut outcomes: Vec<(Process, Outcome)> = Vec::new();

        for proc in &stuck {
            if !self.json {
                print!(
                    "  {} {} [PID {}]... ",
                    "→".bright_black(),
                    proc.name.white(),
                    proc.pid.to_string().cyan()
                );
            }

            let outcome = self.attempt_unstick(proc);

            if !self.json {
                match &outcome {
                    Outcome::Recovered => println!("{}", "recovered".green()),
                    Outcome::Terminated => println!("{}", "terminated".yellow()),
                    Outcome::StillStuck => println!("{}", "still stuck".red()),
                    Outcome::NotStuck => println!("{}", "not stuck".blue()),
                    Outcome::Failed(e) => println!("{}: {}", "failed".red(), e),
                }
            }

            outcomes.push((proc.clone(), outcome));
        }

        // Count outcomes
        let recovered = outcomes
            .iter()
            .filter(|(_, o)| *o == Outcome::Recovered)
            .count();
        let terminated = outcomes
            .iter()
            .filter(|(_, o)| *o == Outcome::Terminated)
            .count();
        let still_stuck = outcomes
            .iter()
            .filter(|(_, o)| *o == Outcome::StillStuck)
            .count();
        let not_stuck = outcomes
            .iter()
            .filter(|(_, o)| *o == Outcome::NotStuck)
            .count();
        let failed = outcomes
            .iter()
            .filter(|(_, o)| matches!(o, Outcome::Failed(_)))
            .count();

        // Output results
        if self.json {
            printer.print_json(&UnstickOutput {
                action: "unstick",
                success: failed == 0 && still_stuck == 0,
                dry_run: false,
                force: self.force,
                found: stuck.len(),
                recovered,
                not_stuck,
                still_stuck,
                terminated,
                failed,
                processes: outcomes
                    .iter()
                    .map(|(p, o)| ProcessOutcome {
                        pid: p.pid,
                        name: p.name.clone(),
                        outcome: match o {
                            Outcome::Recovered => "recovered".to_string(),
                            Outcome::Terminated => "terminated".to_string(),
                            Outcome::StillStuck => "still_stuck".to_string(),
                            Outcome::NotStuck => "not_stuck".to_string(),
                            Outcome::Failed(e) => format!("failed: {}", e),
                        },
                    })
                    .collect(),
            });
        } else {
            println!();
            if recovered > 0 {
                println!(
                    "{} {} process{} recovered",
                    "✓".green().bold(),
                    recovered.to_string().cyan().bold(),
                    if recovered == 1 { "" } else { "es" }
                );
            }
            if not_stuck > 0 {
                println!(
                    "{} {} process{} not stuck",
                    "ℹ".blue().bold(),
                    not_stuck.to_string().cyan().bold(),
                    if not_stuck == 1 { " was" } else { "es were" }
                );
            }
            if terminated > 0 {
                println!(
                    "{} {} process{} terminated",
                    "!".yellow().bold(),
                    terminated.to_string().cyan().bold(),
                    if terminated == 1 { "" } else { "es" }
                );
            }
            if still_stuck > 0 {
                println!(
                    "{} {} process{} still stuck (use --force to terminate)",
                    "✗".red().bold(),
                    still_stuck.to_string().cyan().bold(),
                    if still_stuck == 1 { "" } else { "es" }
                );
            }
            if failed > 0 {
                println!(
                    "{} {} process{} failed",
                    "✗".red().bold(),
                    failed.to_string().cyan().bold(),
                    if failed == 1 { "" } else { "es" }
                );
            }
        }

        Ok(())
    }

    /// Resolve target to processes
    fn resolve_target_processes(&self, target: &str) -> Result<Vec<Process>> {
        resolve_target(target).map_err(|_| ProcError::ProcessNotFound(target.to_string()))
    }

    /// Check if a process appears stuck (high CPU)
    fn is_stuck(&self, proc: &Process) -> bool {
        proc.cpu_percent > 50.0
    }

    /// Attempt to unstick a process using recovery signals
    #[cfg(unix)]
    fn attempt_unstick(&self, proc: &Process) -> Outcome {
        // For targeted processes, check if actually stuck
        if self.target.is_some() && !self.is_stuck(proc) {
            return Outcome::NotStuck;
        }

        let pid = Pid::from_raw(proc.pid as i32);

        // Step 1: SIGCONT (wake if stopped)
        let _ = kill(pid, Signal::SIGCONT);
        std::thread::sleep(Duration::from_secs(1));

        if self.check_recovered(proc) {
            return Outcome::Recovered;
        }

        // Step 2: SIGINT (interrupt)
        if kill(pid, Signal::SIGINT).is_err() {
            if !proc.is_running() {
                return Outcome::Terminated;
            }
        }
        std::thread::sleep(Duration::from_secs(3));

        if !proc.is_running() {
            return Outcome::Terminated;
        }
        if self.check_recovered(proc) {
            return Outcome::Recovered;
        }

        // Without --force, stop here
        if !self.force {
            return Outcome::StillStuck;
        }

        // Step 3: SIGTERM (polite termination) - only with --force
        if proc.terminate().is_err() {
            if !proc.is_running() {
                return Outcome::Terminated;
            }
        }
        std::thread::sleep(Duration::from_secs(5));

        if !proc.is_running() {
            return Outcome::Terminated;
        }

        // Step 4: SIGKILL (force, last resort) - only with --force
        match proc.kill() {
            Ok(()) => Outcome::Terminated,
            Err(e) => {
                if !proc.is_running() {
                    Outcome::Terminated
                } else {
                    Outcome::Failed(e.to_string())
                }
            }
        }
    }

    #[cfg(not(unix))]
    fn attempt_unstick(&self, proc: &Process) -> Outcome {
        // For targeted processes, check if actually stuck
        if self.target.is_some() && !self.is_stuck(proc) {
            return Outcome::NotStuck;
        }

        // On non-Unix, we can only terminate
        if !self.force {
            return Outcome::StillStuck;
        }

        if proc.terminate().is_ok() {
            std::thread::sleep(Duration::from_secs(3));
            if !proc.is_running() {
                return Outcome::Terminated;
            }
        }

        match proc.kill() {
            Ok(()) => Outcome::Terminated,
            Err(e) => Outcome::Failed(e.to_string()),
        }
    }

    /// Check if process has recovered (no longer stuck)
    fn check_recovered(&self, proc: &Process) -> bool {
        if let Ok(Some(current)) = Process::find_by_pid(proc.pid) {
            current.cpu_percent < 10.0
        } else {
            false
        }
    }

    fn show_processes(&self, processes: &[Process]) {
        let label = if self.target.is_some() {
            "Target"
        } else {
            "Found stuck"
        };

        println!(
            "\n{} {} {} process{}:\n",
            "!".yellow().bold(),
            label,
            processes.len().to_string().cyan().bold(),
            if processes.len() == 1 { "" } else { "es" }
        );

        for proc in processes {
            let uptime = proc
                .start_time
                .map(|st| {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs().saturating_sub(st))
                        .unwrap_or(0);
                    format_duration(now)
                })
                .unwrap_or_else(|| "unknown".to_string());

            println!(
                "  {} {} [PID {}] - {:.1}% CPU, running for {}",
                "→".bright_black(),
                proc.name.white().bold(),
                proc.pid.to_string().cyan(),
                proc.cpu_percent,
                uptime.yellow()
            );
        }
    }
}

fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86400, (secs % 86400) / 3600)
    }
}

#[derive(Serialize)]
struct UnstickOutput {
    action: &'static str,
    success: bool,
    dry_run: bool,
    force: bool,
    found: usize,
    recovered: usize,
    not_stuck: usize,
    still_stuck: usize,
    terminated: usize,
    failed: usize,
    processes: Vec<ProcessOutcome>,
}

#[derive(Serialize)]
struct ProcessOutcome {
    pid: u32,
    name: String,
    outcome: String,
}
