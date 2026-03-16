//! `proc orphans` - Find orphaned processes
//!
//! Examples:
//!   proc orphans              # List orphaned processes
//!   proc orphans --in .       # Orphans in current directory
//!   proc orphans --kill       # Find and kill orphans
//!   proc orphans --kill --yes # Kill orphans without confirmation

use crate::core::{resolve_in_dir, Process};
use crate::error::Result;
use crate::ui::{OutputFormat, Printer};
use clap::Args;
use dialoguer::Confirm;
use std::path::PathBuf;

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
        let format = if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };
        let printer = Printer::new(format, self.verbose);

        let mut processes = Process::find_orphans()?;

        // Apply --in and --by filters
        let in_dir_filter = resolve_in_dir(&self.in_dir);
        processes.retain(|p| {
            if let Some(ref dir_path) = in_dir_filter {
                if let Some(ref cwd) = p.cwd {
                    if !PathBuf::from(cwd).starts_with(dir_path) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            if let Some(ref name) = self.by_name {
                if !p.name.to_lowercase().contains(&name.to_lowercase()) {
                    return false;
                }
            }
            true
        });

        if processes.is_empty() {
            printer.success("No orphaned processes found");
            return Ok(());
        }

        printer.warning(&format!(
            "Found {} orphaned process{}",
            processes.len(),
            if processes.len() == 1 { "" } else { "es" }
        ));
        printer.print_processes(&processes);

        // Dry run: show what would be killed
        if self.kill && self.dry_run {
            printer.warning(&format!(
                "Dry run: would kill {} orphaned process{}",
                processes.len(),
                if processes.len() == 1 { "" } else { "es" }
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
