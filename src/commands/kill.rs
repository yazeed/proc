//! `proc kill` - Kill processes
//!
//! Examples:
//!   proc kill node          # Kill all Node.js processes
//!   proc kill :3000         # Kill what's on port 3000
//!   proc kill 1234          # Kill specific PID
//!   proc kill node --yes    # Skip confirmation

use crate::core::port::{parse_port, PortInfo};
use crate::core::Process;
use crate::error::{ProcError, Result};
use crate::ui::{OutputFormat, Printer};
use clap::Args;
use dialoguer::Confirm;

/// Kill process(es)
#[derive(Args, Debug)]
pub struct KillCommand {
    /// Target: process name, PID, or :port
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
}

impl KillCommand {
    pub fn execute(&self) -> Result<()> {
        let format = if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };
        let printer = Printer::new(format, self.verbose);

        // Determine what to kill based on target format
        let processes = self.resolve_target()?;

        if processes.is_empty() {
            return Err(ProcError::ProcessNotFound(self.target.clone()));
        }

        // Dry run: just show what would be killed
        if self.dry_run {
            printer.warning(&format!(
                "Dry run: would kill {} process{}",
                processes.len(),
                if processes.len() == 1 { "" } else { "es" }
            ));
            printer.print_processes(&processes);
            return Ok(());
        }

        // Confirm before killing (unless --yes)
        if !self.yes && !self.json {
            self.print_confirmation_prompt(&processes);

            let confirmed = Confirm::new()
                .with_prompt(format!(
                    "Kill {} process{}?",
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

    /// Resolve the target to a list of processes
    fn resolve_target(&self) -> Result<Vec<Process>> {
        let target = self.target.trim();

        // Check if it's a port (starts with : or is a number in port range)
        if target.starts_with(':') {
            let port = parse_port(target)?;
            return self.find_by_port(port);
        }

        // Check if it's a PID (pure number)
        if let Ok(pid) = target.parse::<u32>() {
            // Could be PID or port - if < 65536, check if it's a port first
            if pid <= 65535 {
                // Try as port first
                if let Ok(processes) = self.find_by_port(pid as u16) {
                    if !processes.is_empty() {
                        return Ok(processes);
                    }
                }
            }
            // Try as PID
            return match Process::find_by_pid(pid)? {
                Some(proc) => Ok(vec![proc]),
                None => Err(ProcError::ProcessNotFound(target.to_string())),
            };
        }

        // Otherwise, treat as process name
        Process::find_by_name(target)
    }

    fn find_by_port(&self, port: u16) -> Result<Vec<Process>> {
        match PortInfo::find_by_port(port)? {
            Some(port_info) => match Process::find_by_pid(port_info.pid)? {
                Some(proc) => Ok(vec![proc]),
                None => Err(ProcError::ProcessGone(port_info.pid)),
            },
            None => Err(ProcError::PortNotFound(port)),
        }
    }

    fn print_confirmation_prompt(&self, processes: &[Process]) {
        use colored::*;

        println!(
            "\n{} Found {} process{} to kill:\n",
            "⚠".yellow().bold(),
            processes.len().to_string().cyan().bold(),
            if processes.len() == 1 { "" } else { "es" }
        );

        for proc in processes {
            println!(
                "  {} {} [PID {}] - CPU: {:.1}%, MEM: {:.1}MB",
                "→".bright_black(),
                proc.name.white().bold(),
                proc.pid.to_string().cyan(),
                proc.cpu_percent,
                proc.memory_mb
            );
        }
        println!();
    }
}
