//! `proc stuck` - Find stuck/hung processes
//!
//! Examples:
//!   proc stuck              # Find processes stuck > 5 minutes
//!   proc stuck --timeout 60 # Find processes stuck > 1 minute
//!   proc stuck --kill       # Find and kill stuck processes

use crate::core::Process;
use crate::error::Result;
use crate::ui::{OutputFormat, Printer};
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

    /// Skip confirmation when killing
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Output as JSON
    #[arg(long, short = 'j')]
    pub json: bool,

    /// Show verbose output
    #[arg(long, short = 'v')]
    pub verbose: bool,
}

impl StuckCommand {
    pub fn execute(&self) -> Result<()> {
        let format = if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };
        let printer = Printer::new(format, self.verbose);

        let timeout = Duration::from_secs(self.timeout);
        let processes = Process::find_stuck(timeout)?;

        if processes.is_empty() {
            printer.success(&format!(
                "No stuck processes found (threshold: {}s)",
                self.timeout
            ));
            return Ok(());
        }

        printer.warning(&format!(
            "Found {} potentially stuck process{}",
            processes.len(),
            if processes.len() == 1 { "" } else { "es" }
        ));
        printer.print_processes(&processes);

        // Kill if requested
        if self.kill {
            if !self.yes && !self.json {
                let confirmed = Confirm::new()
                    .with_prompt(format!(
                        "Kill {} stuck process{}?",
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
