//! `proc orphans` - Find orphaned processes
//!
//! Examples:
//!   proc orphans              # List orphaned processes
//!   proc orphans --in .       # Orphans in current directory
//!   proc orphans --kill       # Find and kill orphans
//!   proc orphans --kill --yes # Kill orphans without confirmation

use crate::core::{apply_filters, Process};
use crate::error::Result;
use crate::ui::{plural, Printer};
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
        let printer = Printer::from_flags(self.json, self.verbose);

        let mut processes = Process::find_orphans()?;

        // Apply --in and --by filters
        apply_filters(&mut processes, &self.in_dir, &self.by_name);

        if processes.is_empty() {
            printer.print_empty_result("orphans", "No orphaned processes found");
            return Ok(());
        }

        printer.warning(&format!(
            "Found {} orphaned process{}",
            processes.len(),
            plural(processes.len())
        ));
        printer.print_processes_as("orphans", &processes, None);

        // Dry run: show what would be killed
        if self.kill && self.dry_run {
            printer.warning(&format!(
                "Dry run: would kill {} orphaned process{}",
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

            printer.print_kill_result(&killed, &failed);
        }

        Ok(())
    }
}
