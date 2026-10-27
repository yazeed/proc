//! `proc free` - Free ports by killing their processes and verifying availability
//!
//! Examples:
//!   proc free :3000              # Kill process on port 3000, verify port freed
//!   proc free :3000,:8080        # Free multiple ports
//!   proc free :3000 --yes        # Skip confirmation
//!   proc free :3000 --wait 30    # Wait up to 30s for port to free

use crate::core::port::PortInfo;
use crate::core::{parse_target, parse_targets, Process, TargetType};
use crate::error::{ExitCode, ProcError, Result};
use crate::ui::{must_confirm, plural, Printer};
use clap::Args;
use colored::*;
use dialoguer::Confirm;
use serde::Serialize;

/// Free port(s) by killing the process and verifying the port is available
#[derive(Args, Debug)]
pub struct FreeCommand {
    /// Port target(s): :port (comma-separated for multiple, e.g. :3000,:8080)
    #[arg(required = true, value_parser = crate::core::validate_target_arg)]
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
        let printer = Printer::from_flags(self.json, self.verbose);

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
            // A port nobody listens on, or whose holder has already gone, is not found
            let holder = match PortInfo::find_by_port(*port)? {
                Some(port_info) => Process::find_by_pid(port_info.pid)?,
                None => None,
            };
            match holder {
                Some(proc) => port_processes.push((*port, proc)),
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
            printer.print_empty_result("free", "All specified ports are already free");
            return Ok(());
        }

        // Deduplicate processes (multiple ports might be held by same process)
        let mut processes: Vec<Process> = {
            let mut seen = std::collections::HashSet::new();
            port_processes
                .iter()
                .filter(|(_, p)| seen.insert(p.pid))
                .map(|(_, p)| p.clone())
                .collect()
        };

        // Real CPU readings whenever the list is shown or returned (dry run,
        // the prompt, or the error for a missing `--yes`); `--yes` acts at once
        if self.dry_run || !self.yes {
            Process::measure_cpu(&mut processes);
        }

        if self.dry_run {
            printer.print_dry_run_with(
                "free",
                &processes,
                &format!(
                    "Dry run: would kill {} process{} to free {} port{}",
                    processes.len(),
                    plural(processes.len()),
                    port_processes.len(),
                    if port_processes.len() == 1 { "" } else { "s" }
                ),
            );
            return Ok(());
        }

        // Same rule everywhere: --yes, or ask; --json never skips it
        if must_confirm("free", self.yes, self.json, &processes)? {
            printer.print_confirmation("free", &processes);

            let prompt = format!(
                "Kill {} process{} to free {} port{}?",
                processes.len(),
                plural(processes.len()),
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

        // SIGTERM first, then SIGKILL for what is left
        let start = std::time::Instant::now();
        stop_all(
            &processes,
            std::time::Duration::from_secs(self.wait.min(5)),
            &printer,
            Process::terminate,
            Process::kill,
        );

        // Poll until ports are free
        let mut freed: Vec<u16> = Vec::new();
        let mut still_busy: Vec<u16> = Vec::new();

        // --wait covers the whole run, graceful exit included, so it is a real
        // maximum; after a force kill the port still gets a second to be released
        let poll_deadline = std::cmp::max(
            start + std::time::Duration::from_secs(self.wait),
            std::time::Instant::now() + std::time::Duration::from_secs(1),
        );

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

            if all_free || std::time::Instant::now() >= poll_deadline {
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

        if still_busy.is_empty() {
            Ok(())
        } else {
            Err(ProcError::reported(
                ExitCode::GeneralError,
                format!(
                    "{} port{} still in use",
                    still_busy.len(),
                    if still_busy.len() == 1 { "" } else { "s" }
                ),
            ))
        }
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

/// Ask each process to end (SIGTERM; on Windows a refused request for a process
/// that is still running is not a failure), give them up to `grace` to do it, then
/// force kill whatever is still running. A signal that fails is a warning, not
/// an error: the port check afterwards says whether it mattered. Returns the
/// PIDs a signal failed for. The signals are parameters so a test can make them
/// fail; the command passes `Process::terminate` and `Process::kill`.
fn stop_all(
    processes: &[Process],
    grace: std::time::Duration,
    printer: &Printer,
    terminate: impl Fn(&Process) -> Result<()>,
    kill: impl Fn(&Process) -> Result<()>,
) -> Vec<u32> {
    let mut failed = Vec::new();
    for proc in processes {
        match terminate(proc) {
            // Windows can't ask a console program to close (taskkill without
            // /F needs a window); the force kill below still stops it
            Err(_) if cfg!(not(unix)) && proc.is_running() => {}
            Err(e) => {
                printer.warning(&format!(
                    "Failed to ask {} [PID {}] to stop: {}",
                    proc.name, proc.pid, e
                ));
                failed.push(proc.pid);
            }
            Ok(()) => {}
        }
    }

    // Wait for graceful exit
    let start = std::time::Instant::now();
    while start.elapsed() < grace {
        if processes.iter().all(|p| !p.is_running()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // Force kill any remaining
    for proc in processes {
        if proc.is_running() {
            if let Err(e) = kill(proc) {
                printer.warning(&format!(
                    "Failed to kill {} [PID {}]: {}",
                    proc.name, proc.pid, e
                ));
                failed.push(proc.pid);
            }
        }
    }
    failed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ProcError;

    /// This test's own process: it exists and is the process it says it is, so
    /// `stop_all` sees it as running. The signals below are stand-ins that
    /// send nothing.
    fn this_process() -> Process {
        Process::find_by_pid(std::process::id())
            .unwrap()
            .expect("this process exists")
    }

    fn refuse(_: &Process) -> Result<()> {
        Err(ProcError::SignalError("not allowed".to_string()))
    }

    #[test]
    fn signals_that_fail_are_reported_not_fatal() {
        let me = this_process();
        let printer = Printer::from_flags(true, false);
        let failed = stop_all(
            std::slice::from_ref(&me),
            std::time::Duration::ZERO,
            &printer,
            refuse,
            refuse,
        );
        // It still counts as running, so the SIGKILL is tried after the polite
        // request; on Windows that request being refused is expected
        if cfg!(unix) {
            assert_eq!(failed, vec![me.pid, me.pid]);
        } else {
            assert_eq!(failed, vec![me.pid]);
        }
    }

    #[test]
    fn signals_that_work_report_nothing() {
        let me = this_process();
        let printer = Printer::from_flags(true, false);
        let failed = stop_all(
            std::slice::from_ref(&me),
            std::time::Duration::ZERO,
            &printer,
            |_| Ok(()),
            |_| Ok(()),
        );
        assert!(failed.is_empty());
    }
}
