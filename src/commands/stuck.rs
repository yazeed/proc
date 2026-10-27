//! `proc stuck` - Find stuck/hung processes
//!
//! A process counts as stuck when it has been running longer than --timeout and,
//! in two readings a second apart, is using more than 50% CPU or is blocked in
//! uninterruptible sleep. A long build at full CPU looks the same as a loop
//! that never ends, so check the list before --kill.
//!
//! Examples:
//!   proc stuck              # Running over 5 minutes, and busy or blocked for 2 seconds
//!   proc stuck --timeout 60 # The same, for processes running over 1 minute
//!   proc stuck --kill       # Find and kill stuck processes

use crate::core::{apply_filters, Process};
use crate::error::Result;
use crate::ui::{must_confirm, plural, Printer};
use clap::Args;
use dialoguer::Confirm;
use std::time::Duration;

/// Find stuck/hung processes
#[derive(Args, Debug)]
pub struct StuckCommand {
    /// Only processes running longer than this many seconds (stuck: over 50% CPU or blocked, for 2 seconds)
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

    /// Print full directories and arguments instead of fitting the terminal width
    #[arg(long, short = 'w')]
    pub wide: bool,

    /// Filter by directory (defaults to current directory if no path given)
    #[arg(long = "in", short = 'i', num_args = 0..=1, default_missing_value = ".")]
    pub in_dir: Option<String>,

    /// Filter by process name
    #[arg(long = "by", short = 'b')]
    pub by_name: Option<String>,
}

impl StuckCommand {
    /// Executes the stuck command, finding long-running processes using high CPU.
    pub fn execute(&self) -> Result<()> {
        let printer = Printer::from_flags(self.json, self.verbose).wide(self.wide);

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

        if !self.kill {
            printer.print_processes_under_headline("stuck", &processes);
            return Ok(());
        }

        // Dry run: show what would be killed
        if self.dry_run {
            printer.print_dry_run_with(
                "stuck",
                &processes,
                &format!(
                    "Dry run: would kill {} stuck process{}",
                    processes.len(),
                    plural(processes.len())
                ),
            );
            return Ok(());
        }

        // Kill: JSON prints a single action result; humans see the list first
        if !self.json {
            printer.print_processes_under_headline("stuck", &processes);
        }

        // Same rule everywhere: --yes, or ask; --json never skips it
        if must_confirm("stuck --kill", self.yes, self.json, &processes)? {
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

        printer.print_action_result_as("stuck", "kill", &killed, &failed)
    }
}
