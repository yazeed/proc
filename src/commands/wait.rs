//! `proc wait` - Wait for process(es) to exit
//!
//! Examples:
//!   proc wait node                   # Wait until all node processes exit
//!   proc wait 1234                   # Wait for PID 1234 to exit
//!   proc wait :3000                  # Wait until port 3000 is free
//!   proc wait node,python            # Wait for all to exit
//!   proc wait node --timeout 3600    # Timeout after 1 hour
//!   proc wait node --interval 10     # Check every 10 seconds

use crate::core::{apply_filters, parse_targets, resolve_targets, Process};
use crate::error::{ProcError, Result};
use crate::ui::{format_duration, plural, Printer};
use clap::Args;
use colored::*;
use serde::Serialize;

/// Wait for process(es) to exit
#[derive(Args, Debug)]
pub struct WaitCommand {
    /// Target(s): process name, PID, or :port (comma-separated for multiple)
    #[arg(required = true)]
    target: String,

    /// Poll interval in seconds
    #[arg(long, short = 'n', default_value = "5")]
    interval: u64,

    /// Timeout in seconds (0 = no timeout)
    #[arg(long, short = 't', default_value = "0")]
    timeout: u64,

    /// Output as JSON
    #[arg(long, short = 'j')]
    json: bool,

    /// Show verbose output
    #[arg(long, short = 'v')]
    verbose: bool,

    /// Filter by directory (defaults to current directory if no path given)
    #[arg(long = "in", short = 'i', num_args = 0..=1, default_missing_value = ".")]
    pub in_dir: Option<String>,

    /// Filter by process name
    #[arg(long = "by", short = 'b')]
    pub by_name: Option<String>,

    /// Suppress periodic status messages (only print final result)
    #[arg(long, short = 'q')]
    quiet: bool,
}

impl WaitCommand {
    /// Executes the wait command, blocking until all matched processes exit.
    pub fn execute(&self) -> Result<()> {
        let printer = Printer::from_flags(self.json, self.verbose);

        // Clamp interval to minimum 1 second
        let interval = self.interval.max(1);

        // Parse and resolve targets
        let targets = parse_targets(&self.target);
        let (mut processes, not_found) = resolve_targets(&targets);

        if !not_found.is_empty() {
            printer.warning(&format!("Not found: {}", not_found.join(", ")));
        }

        // Apply --in and --by filters
        apply_filters(&mut processes, &self.in_dir, &self.by_name);

        if processes.is_empty() {
            return Err(ProcError::ProcessNotFound(self.target.clone()));
        }

        let initial_count = processes.len();
        let start = std::time::Instant::now();

        // Print initial status
        if !self.json {
            println!(
                "{} Waiting for {} process{} to exit...",
                "~".cyan().bold(),
                initial_count.to_string().cyan().bold(),
                plural(initial_count)
            );
            if self.verbose {
                for proc in &processes {
                    println!(
                        "  {} {} [PID {}] - {:.1}% CPU, {}",
                        "->".bright_black(),
                        proc.name.white().bold(),
                        proc.pid.to_string().cyan(),
                        proc.cpu_percent,
                        crate::ui::format_memory(proc.memory_mb)
                    );
                }
            }
        }

        // Track processes: (process, exited_after_seconds)
        let mut tracking: Vec<(Process, Option<u64>)> =
            processes.into_iter().map(|p| (p, None)).collect();

        // Poll loop
        loop {
            std::thread::sleep(std::time::Duration::from_secs(interval));

            let elapsed = start.elapsed().as_secs();

            // Check timeout
            if self.timeout > 0 && elapsed >= self.timeout {
                // Mark still-running processes
                let exited: Vec<ExitedProcess> = tracking
                    .iter()
                    .filter(|(_, t)| t.is_some())
                    .map(|(p, t)| ExitedProcess {
                        pid: p.pid,
                        name: p.name.clone(),
                        exited_after_seconds: t.unwrap(),
                    })
                    .collect();
                let still_running: Vec<RunningProcess> = tracking
                    .iter()
                    .filter(|(_, t)| t.is_none())
                    .map(|(p, _)| RunningProcess {
                        pid: p.pid,
                        name: p.name.clone(),
                    })
                    .collect();

                if self.json {
                    printer.print_json(&WaitOutput {
                        action: "wait",
                        success: false,
                        timed_out: true,
                        elapsed_seconds: elapsed,
                        elapsed_human: format_duration(elapsed),
                        target: self.target.clone(),
                        initial_count,
                        exited,
                        still_running: still_running.clone(),
                    });
                    return Ok(());
                }

                let names: Vec<String> = still_running
                    .iter()
                    .map(|p| format!("{} [{}]", p.name, p.pid))
                    .collect();
                return Err(ProcError::Timeout(format!(
                    "after {} — {} still running: {}",
                    format_duration(elapsed),
                    still_running.len(),
                    names.join(", ")
                )));
            }

            // Check which processes have exited
            for (proc, exited_at) in tracking.iter_mut() {
                if exited_at.is_none() && !proc.is_running() {
                    *exited_at = Some(elapsed);
                    if !self.json {
                        println!(
                            "{} {} [PID {}] exited after {}",
                            "✓".green().bold(),
                            proc.name.white(),
                            proc.pid.to_string().cyan(),
                            format_duration(elapsed)
                        );
                    }
                }
            }

            let still_running_count = tracking.iter().filter(|(_, t)| t.is_none()).count();

            if still_running_count == 0 {
                break;
            }

            // Print periodic status (unless quiet or json)
            if !self.quiet && !self.json {
                let names: Vec<String> = tracking
                    .iter()
                    .filter(|(_, t)| t.is_none())
                    .map(|(p, _)| format!("{} [{}]", p.name, p.pid))
                    .collect();
                let exited_count = initial_count - still_running_count;
                let exited_note = if exited_count > 0 {
                    format!(" ({} exited)", exited_count)
                } else {
                    String::new()
                };
                println!(
                    "{} {} elapsed — {} still running: {}{}",
                    "~".cyan(),
                    format_duration(elapsed),
                    still_running_count,
                    names.join(", "),
                    exited_note.bright_black()
                );
            }
        }

        let elapsed = start.elapsed().as_secs();

        // Final output
        if self.json {
            let exited: Vec<ExitedProcess> = tracking
                .iter()
                .map(|(p, t)| ExitedProcess {
                    pid: p.pid,
                    name: p.name.clone(),
                    exited_after_seconds: t.unwrap_or(elapsed),
                })
                .collect();
            printer.print_json(&WaitOutput {
                action: "wait",
                success: true,
                timed_out: false,
                elapsed_seconds: elapsed,
                elapsed_human: format_duration(elapsed),
                target: self.target.clone(),
                initial_count,
                exited,
                still_running: vec![],
            });
        } else {
            println!(
                "{} All {} process{} exited after {}",
                "✓".green().bold(),
                initial_count.to_string().cyan().bold(),
                plural(initial_count),
                format_duration(elapsed)
            );
        }

        Ok(())
    }
}

#[derive(Serialize)]
struct WaitOutput {
    action: &'static str,
    success: bool,
    timed_out: bool,
    elapsed_seconds: u64,
    elapsed_human: String,
    target: String,
    initial_count: usize,
    exited: Vec<ExitedProcess>,
    still_running: Vec<RunningProcess>,
}

#[derive(Serialize, Clone)]
struct ExitedProcess {
    pid: u32,
    name: String,
    exited_after_seconds: u64,
}

#[derive(Serialize, Clone)]
struct RunningProcess {
    pid: u32,
    name: String,
}
