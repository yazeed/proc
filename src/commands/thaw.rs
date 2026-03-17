//! `proc thaw` - Resume frozen processes with SIGCONT
//!
//! Examples:
//!   proc thaw node              # Resume all frozen node processes
//!   proc thaw :3000             # Resume process on port 3000
//!   proc thaw :3000,:8080       # Resume multiple targets
//!   proc thaw node --yes        # Skip confirmation

#[cfg(unix)]
use crate::core::{parse_targets, resolve_in_dir, resolve_targets_excluding_self};
use crate::error::{ProcError, Result};
#[cfg(unix)]
use crate::ui::{OutputFormat, Printer};
use clap::Args;
#[cfg(unix)]
use dialoguer::Confirm;
#[cfg(unix)]
use std::path::PathBuf;

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

        let format = if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };
        let printer = Printer::new(format, self.verbose);

        let targets = parse_targets(&self.target);
        let (mut processes, not_found) = resolve_targets_excluding_self(&targets);

        if !not_found.is_empty() {
            printer.warning(&format!("Not found: {}", not_found.join(", ")));
        }

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
            return Err(ProcError::ProcessNotFound(self.target.clone()));
        }

        if self.dry_run {
            printer.print_processes(&processes);
            printer.warning(&format!(
                "Dry run: would resume {} process{}",
                processes.len(),
                if processes.len() == 1 { "" } else { "es" }
            ));
            return Ok(());
        }

        if !self.yes && !self.json {
            printer.print_confirmation("resume", &processes);

            let prompt = format!(
                "Resume {} process{}?",
                processes.len(),
                if processes.len() == 1 { "" } else { "es" }
            );

            if !Confirm::new()
                .with_prompt(prompt)
                .default(false)
                .interact()?
            {
                printer.warning("Aborted");
                return Ok(());
            }
        }

        let mut succeeded = Vec::new();
        let mut failed = Vec::new();

        for proc in &processes {
            match proc.send_signal(Signal::SIGCONT) {
                Ok(()) => succeeded.push(proc.clone()),
                Err(e) => failed.push((proc.clone(), e.to_string())),
            }
        }

        printer.print_action_result("Resumed", &succeeded, &failed);

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
