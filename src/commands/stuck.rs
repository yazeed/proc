//! `proc stuck` - Find stuck/hung processes
//!
//! Examples:
//!   proc stuck              # Find processes stuck > 5 minutes
//!   proc stuck --timeout 60 # Find processes stuck > 1 minute
//!   proc stuck --kill       # Find and kill stuck processes

use crate::core::{apply_filters, Process};
use crate::error::Result;
use crate::ui::{plural, Printer};
use clap::Args;
use dialoguer::Confirm;
use std::time::Duration;

/// Find stuck/hung processes
#[derive(Args, Debug)]
pub struct StuckCommand {
    /// Timeout in seconds to consider a process stuck (default: 300 = 5 minutes)
    #[arg(long, short = 't', default_value = "300")]
    pub timeout: u64,

    /// Kill found stuck processes
    #[arg(long, short = 'k')]
    pub kill: bool,

    /// Show what would be killed without actually killing
    #[arg(long)]
    pub dry_run: bool,

    /// Skip confirmation when killing
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Output as JSON
    #[arg(long, short = 'j')]
    pub json: bool,

    /// Show verbose output
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Filter by directory (defaults to current directory if no path given)
    #[arg(long = "in", short = 'i', num_args = 0..=1, default_missing_value = ".")]
    pub in_dir: Option<String>,

    /// Filter by process name
    #[arg(long = "by", short = 'b')]
    pub by_name: Option<String>,
}

impl StuckCommand {
    /// Executes the stuck command, finding processes in uninterruptible states.
    pub fn execute(&self) -> Result<()> {
        let printer = Printer::from_flags(self.json, self.verbose);

        let timeout = Duration::from_secs(self.timeout);
        let mut processes = Process::find_stuck(timeout)?;

        // Apply --in and --by filters
        apply_filters(&mut processes, &self.in_dir, &self.by_name);

        if processes.is_empty() {
            printer.print_empty_result(
                "stuck",
                &format!("No stuck processes found (threshold: {}s)", self.timeout),
            );
            return Ok(());
        }

        printer.warning(&format!(
            "Found {} potentially stuck process{}",
            processes.len(),
            plural(processes.len())
        ));
        printer.print_processes_as("stuck", &processes, None);

        // Dry run: show what would be killed
        if self.kill && self.dry_run {
            printer.print_processes(&processes);
            printer.warning(&format!(
                "Dry run: would kill {} stuck process{}",
                processes.len(),
                plural(processes.len())
            ));
            return Ok(());
        }

        // Kill if requested
        if self.kill {
            if !self.yes && !self.json {
                let confirmed = Confirm::new()
                    .with_prompt(format!(
                        "Kill {} stuck process{}?",
                        processes.len(),
                        plural(processes.len())
                    ))
                    .default(false)
                    .interact()?;

                if !confirmed {
                    printer.warning("Cancelled");
                    return Ok(());
                }
            }

            let mut killed = Vec::new();
            let mut failed = Vec::new();

            for proc in processes {
                // Use kill_and_wait to ensure stuck processes are actually terminated
                match proc.kill_and_wait() {
                    Ok(_) => killed.push(proc),
                    Err(e) => failed.push((proc, e.to_string())),
                }
            }

            printer.print_kill_result(&killed, &failed);
        }

        Ok(())
    }
}
