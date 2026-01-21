//! Stop command - Graceful process termination (SIGTERM)
//!
//! Usage:
//!   proc stop 1234          # Stop PID 1234
//!   proc stop :3000         # Stop process on port 3000
//!   proc stop node          # Stop all node processes

use crate::core::{resolve_target, Process};
use crate::error::{ProcError, Result};
use crate::ui::{OutputFormat, Printer};
use clap::Args;
use dialoguer::Confirm;
use serde::Serialize;

/// Stop process(es) gracefully with SIGTERM
#[derive(Args, Debug)]
pub struct StopCommand {
    /// Target: process name, PID, or :port
    #[arg(required = true)]
    target: String,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    yes: bool,

    /// Output as JSON
    #[arg(long, short)]
    json: bool,

    /// Timeout in seconds to wait before force kill
    #[arg(long, short, default_value = "10")]
    timeout: u64,
}

impl StopCommand {
    /// Executes the stop command, gracefully terminating matched processes.
    pub fn execute(&self) -> Result<()> {
        let format = if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };
        let printer = Printer::new(format, false);

        // Parse target
        let processes = self.find_target_processes()?;

        if processes.is_empty() {
            return Err(ProcError::ProcessNotFound(self.target.clone()));
        }

        // Confirm if not --yes
        if !self.yes && !self.json {
            self.show_processes(&processes);

            let prompt = format!(
                "Stop {} process{}?",
                processes.len(),
                if processes.len() == 1 { "" } else { "es" }
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

        // Stop processes
        let mut stopped = Vec::new();
        let mut failed = Vec::new();

        for proc in &processes {
            match proc.terminate() {
                Ok(()) => {
                    // Wait for process to exit
                    let stopped_gracefully = self.wait_for_exit(proc);
                    if stopped_gracefully {
                        stopped.push(proc.clone());
                    } else {
                        // Force kill after timeout - use kill_and_wait for reliability
                        match proc.kill_and_wait() {
                            Ok(_) => stopped.push(proc.clone()),
                            Err(e) => failed.push((proc.clone(), e.to_string())),
                        }
                    }
                }
                Err(e) => failed.push((proc.clone(), e.to_string())),
            }
        }

        // Output results
        if self.json {
            printer.print_json(&StopOutput {
                action: "stop",
                success: failed.is_empty(),
                stopped_count: stopped.len(),
                failed_count: failed.len(),
                stopped: &stopped,
                failed: &failed
                    .iter()
                    .map(|(p, e)| FailedStop {
                        process: p,
                        error: e,
                    })
                    .collect::<Vec<_>>(),
            });
        } else {
            self.print_results(&printer, &stopped, &failed);
        }

        Ok(())
    }

    fn find_target_processes(&self) -> Result<Vec<Process>> {
        resolve_target(&self.target)
    }

    fn wait_for_exit(&self, proc: &Process) -> bool {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(self.timeout);

        while start.elapsed() < timeout {
            if !proc.is_running() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        false
    }

    fn show_processes(&self, processes: &[Process]) {
        use colored::*;

        println!(
            "\n{} Found {} process{}:\n",
            "!".yellow().bold(),
            processes.len().to_string().cyan().bold(),
            if processes.len() == 1 { "" } else { "es" }
        );

        for proc in processes {
            println!(
                "  {} {} [PID {}] - {:.1}% CPU, {:.1} MB",
                "→".bright_black(),
                proc.name.white().bold(),
                proc.pid.to_string().cyan(),
                proc.cpu_percent,
                proc.memory_mb
            );
        }
        println!();
    }

    fn print_results(&self, printer: &Printer, stopped: &[Process], failed: &[(Process, String)]) {
        use colored::*;

        if !stopped.is_empty() {
            println!(
                "{} Stopped {} process{}",
                "✓".green().bold(),
                stopped.len().to_string().cyan().bold(),
                if stopped.len() == 1 { "" } else { "es" }
            );
            for proc in stopped {
                println!(
                    "  {} {} [PID {}]",
                    "→".bright_black(),
                    proc.name.white(),
                    proc.pid.to_string().cyan()
                );
            }
        }

        if !failed.is_empty() {
            printer.error(&format!(
                "Failed to stop {} process{}",
                failed.len(),
                if failed.len() == 1 { "" } else { "es" }
            ));
            for (proc, err) in failed {
                println!(
                    "  {} {} [PID {}]: {}",
                    "→".bright_black(),
                    proc.name.white(),
                    proc.pid.to_string().cyan(),
                    err.red()
                );
            }
        }
    }
}

#[derive(Serialize)]
struct StopOutput<'a> {
    action: &'static str,
    success: bool,
    stopped_count: usize,
    failed_count: usize,
    stopped: &'a [Process],
    failed: &'a [FailedStop<'a>],
}

#[derive(Serialize)]
struct FailedStop<'a> {
    process: &'a Process,
    error: &'a str,
}
