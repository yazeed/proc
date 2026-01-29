//! `proc kill` - Kill processes
//!
//! Examples:
//!   proc kill node              # Kill all Node.js processes
//!   proc kill :3000             # Kill what's on port 3000
//!   proc kill 1234              # Kill specific PID
//!   proc kill :3000,:8080       # Kill multiple targets
//!   proc kill :3000,1234,node   # Mixed targets (port + PID + name)
//!   proc kill node --yes        # Skip confirmation

use crate::core::{parse_targets, resolve_targets, Process};
use crate::error::{ProcError, Result};
use crate::ui::{OutputFormat, Printer};
use clap::Args;
use dialoguer::Confirm;

/// Kill process(es)
#[derive(Args, Debug)]
pub struct KillCommand {
    /// Target(s): process name, PID, or :port (comma-separated for multiple)
    pub target: String,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Show what would be killed without actually killing
    #[arg(long)]
    pub dry_run: bool,

    /// Output as JSON
    #[arg(long, short = 'j')]
    pub json: bool,

    /// Show verbose output
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Send SIGTERM instead of SIGKILL (graceful)
    #[arg(long, short = 'g')]
    pub graceful: bool,
}

impl KillCommand {
    /// Executes the kill command, forcefully terminating matched processes.
    pub fn execute(&self) -> Result<()> {
        let format = if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };
        let printer = Printer::new(format, self.verbose);

        // Parse comma-separated targets and resolve to processes
        let targets = parse_targets(&self.target);
        let (processes, not_found) = resolve_targets(&targets);

        // Warn about targets that weren't found
        for target in &not_found {
            printer.warning(&format!("Target not found: {}", target));
        }

        if processes.is_empty() {
            return Err(ProcError::ProcessNotFound(self.target.clone()));
        }

        // Dry run: just show what would be killed
        if self.dry_run {
            printer.warning(&format!(
                "Dry run: would kill {} process{}",
                processes.len(),
                if processes.len() == 1 { "" } else { "es" }
            ));
            printer.print_processes(&processes);
            return Ok(());
        }

        // Confirm before killing (unless --yes)
        if !self.yes && !self.json {
            self.print_confirmation_prompt(&processes);

            let confirmed = Confirm::new()
                .with_prompt(format!(
                    "Kill {} process{}?",
                    processes.len(),
                    if processes.len() == 1 { "" } else { "es" }
                ))
                .default(false)
                .interact()
                .unwrap_or(false);

            if !confirmed {
                printer.warning("Cancelled");
                return Ok(());
            }
        }

        // Kill the processes
        let mut killed = Vec::new();
        let mut failed = Vec::new();

        for proc in processes {
            let result = if self.graceful {
                proc.terminate()
            } else {
                proc.kill()
            };

            match result {
                Ok(()) => killed.push(proc),
                Err(e) => failed.push((proc, e.to_string())),
            }
        }

        printer.print_kill_result(&killed, &failed);

        if failed.is_empty() {
            Ok(())
        } else {
            Err(ProcError::SignalError(format!(
                "Failed to kill {} process(es)",
                failed.len()
            )))
        }
    }

    fn print_confirmation_prompt(&self, processes: &[Process]) {
        use colored::*;

        println!(
            "\n{} Found {} process{} to kill:\n",
            "⚠".yellow().bold(),
            processes.len().to_string().cyan().bold(),
            if processes.len() == 1 { "" } else { "es" }
        );

        for proc in processes {
            println!(
                "  {} {} [PID {}] - CPU: {:.1}%, MEM: {:.1}MB",
                "→".bright_black(),
                proc.name.white().bold(),
                proc.pid.to_string().cyan(),
                proc.cpu_percent,
                proc.memory_mb
            );
        }
        println!();
    }
}
