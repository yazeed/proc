//! `proc kill` - Kill processes
//!
//! Examples:
//!   proc kill node              # Kill all Node.js processes
//!   proc kill :3000             # Kill what's on port 3000
//!   proc kill 1234              # Kill specific PID
//!   proc kill :3000,:8080       # Kill multiple targets
//!   proc kill :3000,1234,node   # Mixed targets (port + PID + name)
//!   proc kill node --yes        # Skip confirmation

use crate::core::{apply_filters, parse_targets, resolve_targets_excluding_self};
use crate::error::{ProcError, Result};
use crate::ui::Printer;
use clap::Args;

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

    /// Filter by directory (defaults to current directory if no path given)
    #[arg(long = "in", short = 'i', num_args = 0..=1, default_missing_value = ".")]
    pub in_dir: Option<String>,

    /// Filter by process name
    #[arg(long = "by", short = 'b')]
    pub by_name: Option<String>,
}

impl KillCommand {
    /// Executes the kill command, forcefully terminating matched processes.
    pub fn execute(&self) -> Result<()> {
        let printer = Printer::from_flags(self.json, self.verbose);

        // Parse comma-separated targets and resolve to processes
        // Use resolve_targets_excluding_self to avoid killing ourselves
        let targets = parse_targets(&self.target);
        let (mut processes, not_found) = resolve_targets_excluding_self(&targets);

        // Warn about targets that weren't found
        if !not_found.is_empty() {
            printer.warning(&format!("Not found: {}", not_found.join(", ")));
        }

        // Apply --in and --by filters
        apply_filters(&mut processes, &self.in_dir, &self.by_name);

        if processes.is_empty() {
            return Err(ProcError::ProcessNotFound(self.target.clone()));
        }

        // Dry run: just show what would be killed
        if self.dry_run {
            printer.print_dry_run("kill", &processes);
            return Ok(());
        }

        // Confirm before killing (unless --yes)
        if !printer.ask_confirm("kill", &processes, self.yes)? {
            return Ok(());
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
}
