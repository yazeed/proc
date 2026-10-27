//! `proc orphans` - Find orphaned processes
//!
//! Examples:
//!   proc orphans              # List orphaned processes
//!   proc orphans --in .       # Orphans in current directory
//!   proc orphans --kill       # Find and kill orphans
//!   proc orphans --kill --yes # Kill orphans without confirmation

use crate::core::{apply_filters, Process};
use crate::error::Result;
use crate::ui::{must_confirm, plural, Printer};
use clap::Args;
use dialoguer::Confirm;

/// Find orphaned processes (parent has exited)
#[derive(Args, Debug)]
pub struct OrphansCommand {
    /// Kill found orphaned processes
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

impl OrphansCommand {
    /// Executes the orphans command, finding processes whose parent has exited.
    pub fn execute(&self) -> Result<()> {
        let printer = Printer::from_flags(self.json, self.verbose).wide(self.wide);

        let mut processes = Process::find_orphans()?;

        // Apply --in and --by filters
        apply_filters(&mut processes, &self.in_dir, &self.by_name);

        // Real CPU readings for every view that shows the list; only
        // `--kill --json` (no dry run) goes straight to killing
        if !(self.kill && self.json && !self.dry_run) {
            Process::measure_cpu(&mut processes);
        }

        if processes.is_empty() {
            printer.print_empty_result("orphans", "No orphaned processes found");
            return Ok(());
        }

        printer.warning(&format!(
            "Found {} orphaned process{}",
            processes.len(),
            plural(processes.len())
        ));

        if !self.kill {
            printer.print_processes_under_headline("orphans", &processes);
            return Ok(());
        }

        // Dry run: show what would be killed
        if self.dry_run {
            printer.print_dry_run_with(
                "orphans",
                &processes,
                &format!(
                    "Dry run: would kill {} orphaned process{}",
                    processes.len(),
                    plural(processes.len())
                ),
            );
            return Ok(());
        }

        // Kill: JSON prints a single action result; humans see the list first
        if !self.json {
            printer.print_processes_under_headline("orphans", &processes);
        }

        // Same rule everywhere: --yes, or ask; --json never skips it
        if must_confirm("orphans --kill", self.yes, self.json)? {
            let confirmed = Confirm::new()
                .with_prompt(format!(
                    "Kill {} orphaned process{}?",
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
            match proc.kill_and_wait() {
                Ok(_) => killed.push(proc),
                Err(e) => failed.push((proc, e.to_string())),
            }
        }

        printer.print_action_result_as("orphans", "kill", &killed, &failed)
    }
}
