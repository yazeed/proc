//! `proc freeze` - Pause processes with SIGSTOP
//!
//! Examples:
//!   proc freeze node              # Freeze all node processes
//!   proc freeze :3000             # Freeze process on port 3000
//!   proc freeze :3000,:8080       # Freeze multiple targets
//!   proc freeze node --yes        # Skip confirmation
//!   proc freeze node --dry-run    # Show what would be frozen

#[cfg(unix)]
use crate::core::{apply_filters, parse_targets, resolve_targets_excluding_self};
use crate::error::{ProcError, Result};
#[cfg(unix)]
use crate::ui::Printer;
use clap::Args;

/// Freeze (pause) process(es) with SIGSTOP
#[derive(Args, Debug)]
pub struct FreezeCommand {
    /// Target(s): process name, PID, or :port (comma-separated for multiple)
    #[arg(required = true)]
    pub target: String,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Show what would be frozen without actually freezing
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

impl FreezeCommand {
    /// Executes the freeze command, pausing matched processes with SIGSTOP.
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
            printer.print_dry_run("freeze", &processes);
            return Ok(());
        }

        if !printer.ask_confirm("freeze", &processes, self.yes)? {
            return Ok(());
        }

        let mut succeeded = Vec::new();
        let mut failed = Vec::new();

        for proc in &processes {
            match proc.send_signal(Signal::SIGSTOP) {
                Ok(()) => succeeded.push(proc.clone()),
                Err(e) => failed.push((proc.clone(), e.to_string())),
            }
        }

        printer.print_action_result("freeze", &succeeded, &failed);

        Ok(())
    }

    /// Windows stub
    #[cfg(not(unix))]
    pub fn execute(&self) -> Result<()> {
        Err(ProcError::NotSupported(
            "freeze (SIGSTOP) is not supported on Windows".to_string(),
        ))
    }
}
