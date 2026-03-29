//! `proc thaw` - Resume frozen processes with SIGCONT
//!
//! Examples:
//!   proc thaw node              # Resume all frozen node processes
//!   proc thaw :3000             # Resume process on port 3000
//!   proc thaw :3000,:8080       # Resume multiple targets
//!   proc thaw node --yes        # Skip confirmation

#[cfg(unix)]
use crate::core::{apply_filters, parse_targets, resolve_targets_excluding_self};
use crate::error::{ProcError, Result};
#[cfg(unix)]
use crate::ui::Printer;
use clap::Args;

/// Resume frozen process(es) with SIGCONT
#[derive(Args, Debug)]
pub struct ThawCommand {
    /// Target(s): process name, PID, or :port (comma-separated for multiple)
    #[arg(required = true)]
    pub target: String,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Show what would be resumed without actually resuming
    #[arg(long)]
    pub dry_run: bool,

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

impl ThawCommand {
    /// Executes the thaw command, resuming frozen processes with SIGCONT.
    #[cfg(unix)]
    pub fn execute(&self) -> Result<()> {
        use nix::sys::signal::Signal;

        let printer = Printer::from_flags(self.json, self.verbose);

        let targets = parse_targets(&self.target);
        let (mut processes, not_found) = resolve_targets_excluding_self(&targets);

        if !not_found.is_empty() {
            printer.warning(&format!("Not found: {}", not_found.join(", ")));
        }

        // Apply --in and --by filters
        apply_filters(&mut processes, &self.in_dir, &self.by_name);

        if processes.is_empty() {
            return Err(ProcError::ProcessNotFound(self.target.clone()));
        }

        if self.dry_run {
            printer.print_dry_run("resume", &processes);
            return Ok(());
        }

        if !printer.ask_confirm("resume", &processes, self.yes)? {
            return Ok(());
        }

        let mut succeeded = Vec::new();
        let mut failed = Vec::new();

        for proc in &processes {
            match proc.send_signal(Signal::SIGCONT) {
                Ok(()) => succeeded.push(proc.clone()),
                Err(e) => failed.push((proc.clone(), e.to_string())),
            }
        }

        printer.print_action_result("resume", &succeeded, &failed);

        Ok(())
    }

    /// Windows stub
    #[cfg(not(unix))]
    pub fn execute(&self) -> Result<()> {
        Err(ProcError::NotSupported(
            "thaw (SIGCONT) is not supported on Windows".to_string(),
        ))
    }
}
