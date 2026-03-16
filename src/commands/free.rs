//! `proc free` - Free ports by killing their processes and verifying availability
//!
//! Examples:
//!   proc free :3000              # Kill process on port 3000, verify port freed
//!   proc free :3000,:8080        # Free multiple ports
//!   proc free :3000 --yes        # Skip confirmation
//!   proc free :3000 --wait 30    # Wait up to 30s for port to free

use crate::core::port::PortInfo;
use crate::core::{parse_target, parse_targets, Process, TargetType};
use crate::error::{ProcError, Result};
use crate::ui::{OutputFormat, Printer};
use clap::Args;
use colored::*;
use dialoguer::Confirm;
use serde::Serialize;

/// Free port(s) by killing the process and verifying the port is available
#[derive(Args, Debug)]
pub struct FreeCommand {
    /// Port target(s): :port (comma-separated for multiple, e.g. :3000,:8080)
    #[arg(required = true)]
    pub target: String,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Show what would be freed without actually freeing
    #[arg(long)]
    pub dry_run: bool,

    /// Output as JSON
    #[arg(long, short = 'j')]
    pub json: bool,

    /// Show verbose output
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Max seconds to wait for port to become free
    #[arg(long, default_value = "10")]
    pub wait: u64,
}

impl FreeCommand {
    /// Executes the free command, killing processes and verifying ports are freed.
    pub fn execute(&self) -> Result<()> {
        let format = if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };
        let printer = Printer::new(format, self.verbose);

        let targets = parse_targets(&self.target);

        // Validate all targets are ports
        let mut ports: Vec<u16> = Vec::new();
        for target in &targets {
            match parse_target(target) {
                TargetType::Port(port) => ports.push(port),
                _ => {
                    return Err(ProcError::InvalidInput(format!(
                        "proc free only works with port targets (e.g. :3000), got '{}'. Use proc kill for processes.",
                        target
                    )));
                }
            }
        }

        // Resolve ports to processes
        let mut port_processes: Vec<(u16, Process)> = Vec::new();
        let mut not_found: Vec<u16> = Vec::new();

        for port in &ports {
            match PortInfo::find_by_port(*port)? {
                Some(port_info) => match Process::find_by_pid(port_info.pid)? {
                    Some(proc) => port_processes.push((*port, proc)),
                    None => not_found.push(*port),
                },
                None => not_found.push(*port),
            }
        }

        for port in &not_found {
            printer.warning(&format!("No process listening on port {}", port));
        }

        if port_processes.is_empty() {
            if not_found.is_empty() {
                return Err(ProcError::InvalidInput(
                    "No port targets specified".to_string(),
                ));
            }
            printer.success("All specified ports are already free");
            return Ok(());
        }

        // Deduplicate processes (multiple ports might be held by same process)
        let processes: Vec<Process> = {
            let mut seen = std::collections::HashSet::new();
            port_processes
                .iter()
                .filter(|(_, p)| seen.insert(p.pid))
                .map(|(_, p)| p.clone())
                .collect()
        };

        if self.dry_run {
            printer.print_processes(&processes);
            printer.warning(&format!(
                "Dry run: would kill {} process{} to free {} port{}",
                processes.len(),
                if processes.len() == 1 { "" } else { "es" },
                port_processes.len(),
                if port_processes.len() == 1 { "" } else { "s" }
            ));
            return Ok(());
        }

        if !self.yes && !self.json {
            printer.print_confirmation("free", &processes);

            let prompt = format!(
                "Kill {} process{} to free {} port{}?",
                processes.len(),
                if processes.len() == 1 { "" } else { "es" },
                port_processes.len(),
                if port_processes.len() == 1 { "" } else { "s" }
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

        // Terminate processes (SIGTERM first)
        let mut kill_failed = Vec::new();
        for proc in &processes {
            if let Err(e) = proc.terminate() {
                printer.warning(&format!(
                    "Failed to send SIGTERM to {} [PID {}]: {}",
                    proc.name, proc.pid, e
                ));
                kill_failed.push(proc.pid);
            }
        }

        // Wait for graceful exit
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(self.wait.min(5));
        while start.elapsed() < timeout {
            if processes.iter().all(|p| !p.is_running()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        // Force kill any remaining
        for proc in &processes {
            if proc.is_running() {
                if let Err(e) = proc.kill() {
                    printer.warning(&format!(
                        "Failed to kill {} [PID {}]: {}",
                        proc.name, proc.pid, e
                    ));
                    kill_failed.push(proc.pid);
                }
            }
        }

        // Poll until ports are free
        let mut freed: Vec<u16> = Vec::new();
        let mut still_busy: Vec<u16> = Vec::new();

        let poll_start = std::time::Instant::now();
        let poll_timeout = std::time::Duration::from_secs(self.wait);

        let target_ports: Vec<u16> = port_processes.iter().map(|(port, _)| *port).collect();

        loop {
            let mut all_free = true;
            freed.clear();
            still_busy.clear();

            for port in &target_ports {
                match PortInfo::find_by_port(*port) {
                    Ok(None) => freed.push(*port),
                    _ => {
                        still_busy.push(*port);
                        all_free = false;
                    }
                }
            }

            if all_free || poll_start.elapsed() >= poll_timeout {
                break;
            }

            std::thread::sleep(std::time::Duration::from_millis(250));
        }

        // Report results
        if self.json {
            let results: Vec<FreeResult> = freed
                .iter()
                .map(|p| FreeResult {
                    port: *p,
                    freed: true,
                })
                .chain(still_busy.iter().map(|p| FreeResult {
                    port: *p,
                    freed: false,
                }))
                .collect();

            printer.print_json(&FreeOutput {
                action: "free",
                success: still_busy.is_empty(),
                results,
            });
        } else {
            for port in &freed {
                println!(
                    "{} Freed port {}",
                    "✓".green().bold(),
                    port.to_string().cyan().bold()
                );
            }
            for port in &still_busy {
                println!(
                    "{} Port {} still in use (may be in TIME_WAIT)",
                    "✗".red().bold(),
                    port.to_string().cyan()
                );
            }
        }

        Ok(())
    }
}

#[derive(Serialize)]
struct FreeOutput {
    action: &'static str,
    success: bool,
    results: Vec<FreeResult>,
}

#[derive(Serialize)]
struct FreeResult {
    port: u16,
    freed: bool,
}
