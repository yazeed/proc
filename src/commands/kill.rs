//! `proc kill` - Kill processes
//!
//! Examples:
//!   proc kill node              # Kill all Node.js processes
//!   proc kill :3000             # Kill what's on port 3000
//!   proc kill 1234              # Kill specific PID
//!   proc kill 1234 5678         # Kill several targets: separate arguments
//!   proc kill :3000 :8080 node  # Mixed targets (port + port + name)
//!   proc kill node --yes        # Skip confirmation
//!
//! Commas still work (`proc kill :3000,:8080`) but are deprecated.

use crate::core::{
    apply_filters, apply_to_each, flatten_targets, resolve_targets_excluding_self, Process,
};
use crate::error::{ProcError, Result};
use crate::ui::Printer;
use clap::Args;

/// Kill process(es)
#[derive(Args, Debug)]
pub struct KillCommand {
    /// Target(s): process name, PID, or :port (separate arguments for several)
    #[arg(required = true, num_args = 1.., value_parser = crate::core::validate_target_arg)]
    pub target: Vec<String>,

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

    /// Filter by directory; takes the next word as the directory unless it starts with '-'
    /// (bare --in is the current directory, --in=DIR is explicit)
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

        // Split each value on commas and resolve to processes
        // Use resolve_targets_excluding_self to avoid killing ourselves
        let targets = flatten_targets(&self.target);
        let (mut processes, not_found) = resolve_targets_excluding_self(&targets);

        // Warn about targets that weren't found
        if !not_found.is_empty() {
            printer.warning(&format!("Not found: {}", not_found.join(", ")));
        }

        // Apply --in and --by filters
        apply_filters(&mut processes, &self.in_dir, &self.by_name);

        // Real CPU readings whenever the list is shown or returned (dry run, the
        // confirmation prompt, or the error for a missing `--yes`); only `--yes`
        // acts at once, without the wait
        if self.dry_run || !self.yes {
            Process::measure_cpu(&mut processes);
        }

        if processes.is_empty() {
            return Err(ProcError::ProcessNotFound(self.target.join(" ")));
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
        let (killed, failed) = apply_to_each(&processes, |proc| {
            if self.graceful {
                proc.terminate()
            } else {
                proc.kill()
            }
        });

        printer.print_action_result("kill", &killed, &failed)
    }
}
