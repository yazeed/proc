//! `proc on` - Port/process lookup
//!
//! Usage:
//!   proc on :3000              # What process is on port 3000?
//!   proc on :3000,:8080        # What's on multiple ports?
//!   proc on 1234               # What ports is PID 1234 listening on?
//!   proc on node               # What ports are node processes listening on?
//!   proc on node --in .        # Node processes in cwd and their ports

use crate::core::{
    find_ports_for_pid, parse_target, parse_targets, resolve_in_dir, resolve_target, PortInfo,
    Process, TargetType,
};
use crate::error::{ProcError, Result};
use crate::ui::{format_duration, format_memory, Printer};
use clap::Args;
use colored::*;
use serde::Serialize;
use std::path::PathBuf;

/// Show what's on a port, or what ports a process is on
#[derive(Args, Debug)]
pub struct OnCommand {
    /// Target(s): :port, PID, or process name (comma-separated for multiple)
    pub target: String,

    /// Filter by directory (defaults to current directory if no path given)
    #[arg(long = "in", short = 'i', num_args = 0..=1, default_missing_value = ".")]
    pub in_dir: Option<String>,

    /// Filter by process name
    #[arg(long = "by", short = 'b')]
    pub by_name: Option<String>,

    /// Output as JSON
    #[arg(long, short = 'j')]
    pub json: bool,

    /// Show verbose output (full command line)
    #[arg(long, short = 'v')]
    pub verbose: bool,
}

impl OnCommand {
    /// Executes the on command, performing bidirectional port/process lookup.
    pub fn execute(&self) -> Result<()> {
        let targets = parse_targets(&self.target);

        // For single target, use original behavior
        if targets.len() == 1 {
            return match parse_target(&targets[0]) {
                TargetType::Port(port) => self.show_process_on_port(port),
                TargetType::Pid(pid) => self.show_ports_for_pid(pid),
                TargetType::Name(name) => self.show_ports_for_name(&name),
            };
        }

        // Multi-target handling
        let mut not_found = Vec::new();

        for target in &targets {
            match parse_target(target) {
                TargetType::Port(port) => {
                    if let Err(e) = self.show_process_on_port(port) {
                        if !self.json {
                            println!("{} Port {}: {}", "⚠".yellow(), port, e);
                        }
                        not_found.push(target.clone());
                    }
                }
                TargetType::Pid(pid) => {
                    if let Err(e) = self.show_ports_for_pid(pid) {
                        if !self.json {
                            println!("{} PID {}: {}", "⚠".yellow(), pid, e);
                        }
                        not_found.push(target.clone());
                    }
                }
                TargetType::Name(ref name) => {
                    if let Err(e) = self.show_ports_for_name(name) {
                        if !self.json {
                            println!("{} '{}': {}", "⚠".yellow(), name, e);
                        }
                        not_found.push(target.clone());
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if process matches --in filter
    fn matches_in_filter(&self, proc: &Process) -> bool {
        if let Some(ref dir_path) = resolve_in_dir(&self.in_dir) {
            if let Some(ref proc_cwd) = proc.cwd {
                let proc_path = PathBuf::from(proc_cwd);
                proc_path.starts_with(dir_path)
            } else {
                false
            }
        } else {
            true
        }
    }

    /// Check if process matches --by filter
    fn matches_by_filter(&self, proc: &Process) -> bool {
        if let Some(ref name) = self.by_name {
            crate::core::matches_by_filter(proc, name)
        } else {
            true
        }
    }

    /// Check if process matches all filters (--in and --by)
    fn matches_filters(&self, proc: &Process) -> bool {
        self.matches_in_filter(proc) && self.matches_by_filter(proc)
    }

    /// Show what process is on a specific port
    fn show_process_on_port(&self, port: u16) -> Result<()> {
        let port_info = match PortInfo::find_by_port(port)? {
            Some(info) => info,
            None => return Err(ProcError::PortNotFound(port)),
        };

        let process = Process::find_by_pid(port_info.pid)?;

        // Apply --in and --by filters if present
        if let Some(ref proc) = process {
            if !self.matches_filters(proc) {
                return Err(ProcError::ProcessNotFound(format!(
                    "port {} (process not in specified directory)",
                    port
                )));
            }
        }

        if self.json {
            let printer = Printer::from_flags(true, self.verbose);
            let output = PortLookupOutput {
                action: "on",
                query_type: "port_to_process",
                success: true,
                port: Some(port_info.port),
                protocol: Some(format!("{:?}", port_info.protocol).to_lowercase()),
                address: port_info.address.clone(),
                process: process.as_ref(),
                ports: None,
            };
            printer.print_json(&output);
        } else {
            self.print_process_on_port(&port_info, process.as_ref());
        }

        Ok(())
    }

    /// Show what ports a PID is listening on
    fn show_ports_for_pid(&self, pid: u32) -> Result<()> {
        let process = Process::find_by_pid(pid)?
            .ok_or_else(|| ProcError::ProcessNotFound(pid.to_string()))?;

        // Apply --in and --by filters if present
        if !self.matches_filters(&process) {
            return Err(ProcError::ProcessNotFound(format!(
                "PID {} (not in specified directory)",
                pid
            )));
        }

        let ports = find_ports_for_pid(pid)?;

        if self.json {
            let printer = Printer::from_flags(true, self.verbose);
            let output = PortLookupOutput {
                action: "on",
                query_type: "process_to_ports",
                success: true,
                port: None,
                protocol: None,
                address: None,
                process: Some(&process),
                ports: Some(&ports),
            };
            printer.print_json(&output);
        } else {
            self.print_ports_for_process(&process, &ports);
        }

        Ok(())
    }

    /// Show what ports processes with a given name are listening on
    fn show_ports_for_name(&self, name: &str) -> Result<()> {
        let mut processes = resolve_target(name)?;

        if processes.is_empty() {
            return Err(ProcError::ProcessNotFound(name.to_string()));
        }

        // Apply --in and --by filters if present
        if self.in_dir.is_some() || self.by_name.is_some() {
            processes.retain(|p| self.matches_filters(p));
            if processes.is_empty() {
                return Err(ProcError::ProcessNotFound(format!(
                    "'{}' (no matches with specified filters)",
                    name
                )));
            }
        }

        let mut all_results: Vec<(Process, Vec<PortInfo>)> = Vec::new();

        for proc in processes {
            let ports = find_ports_for_pid(proc.pid)?;
            all_results.push((proc, ports));
        }

        if self.json {
            let printer = Printer::from_flags(true, self.verbose);
            let results: Vec<_> = all_results
                .iter()
                .map(|(proc, ports)| ProcessPortsJson {
                    process: proc,
                    ports,
                })
                .collect();
            let output = MultiProcessPortsOutput {
                action: "on",
                success: true,
                count: results.len(),
                results: &results,
            };
            printer.print_json(&output);
        } else {
            for (proc, ports) in &all_results {
                self.print_ports_for_process(proc, ports);
            }
        }

        Ok(())
    }

    fn print_process_on_port(&self, port_info: &PortInfo, process: Option<&Process>) {
        println!(
            "{} Port {} is used by:",
            "✓".green().bold(),
            port_info.port.to_string().cyan().bold()
        );
        println!();

        println!(
            "  {} {} (PID {})",
            "Process:".bright_black(),
            port_info.process_name.white().bold(),
            port_info.pid.to_string().cyan()
        );

        if let Some(proc) = process {
            if let Some(ref cwd) = proc.cwd {
                println!("  {} {}", "Directory:".bright_black(), cwd);
            }
            if let Some(ref path) = proc.exe_path {
                println!("  {} {}", "Path:".bright_black(), path.bright_black());
            }
        }

        let addr = port_info.address.as_deref().unwrap_or("*");
        println!(
            "  {} {} on {}",
            "Listening:".bright_black(),
            format!("{:?}", port_info.protocol).to_uppercase(),
            addr
        );

        if let Some(proc) = process {
            println!(
                "  {} {:.1}% CPU, {}",
                "Resources:".bright_black(),
                proc.cpu_percent,
                format_memory(proc.memory_mb)
            );

            if let Some(start_time) = proc.start_time {
                let uptime = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs().saturating_sub(start_time))
                    .unwrap_or(0);
                println!("  {} {}", "Uptime:".bright_black(), format_duration(uptime));
            }

            if self.verbose {
                if let Some(ref cmd) = proc.command {
                    println!("  {} {}", "Command:".bright_black(), cmd.bright_black());
                }
            }
        }

        println!();
    }

    fn print_ports_for_process(&self, process: &Process, ports: &[PortInfo]) {
        println!(
            "{} {} (PID {}) is listening on:",
            "✓".green().bold(),
            process.name.white().bold(),
            process.pid.to_string().cyan().bold()
        );
        println!();

        if ports.is_empty() {
            println!("  {} No listening ports", "ℹ".blue());
        } else {
            for port_info in ports {
                let addr = port_info.address.as_deref().unwrap_or("*");
                println!(
                    "  {} :{} ({} on {})",
                    "→".bright_black(),
                    port_info.port.to_string().cyan(),
                    format!("{:?}", port_info.protocol).to_uppercase(),
                    addr
                );
            }
        }

        if let Some(ref cwd) = process.cwd {
            println!();
            println!("  {} {}", "Directory:".bright_black(), cwd);
        }

        if self.verbose {
            if let Some(ref path) = process.exe_path {
                println!("  {} {}", "Path:".bright_black(), path.bright_black());
            }
            if let Some(ref cmd) = process.command {
                println!("  {} {}", "Command:".bright_black(), cmd.bright_black());
            }
        }

        println!();
    }
}

#[derive(Serialize)]
struct PortLookupOutput<'a> {
    action: &'static str,
    query_type: &'static str,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    process: Option<&'a Process>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ports: Option<&'a [PortInfo]>,
}

#[derive(Serialize)]
struct ProcessPortsJson<'a> {
    process: &'a Process,
    ports: &'a [PortInfo],
}

#[derive(Serialize)]
struct MultiProcessPortsOutput<'a> {
    action: &'static str,
    success: bool,
    count: usize,
    results: &'a Vec<ProcessPortsJson<'a>>,
}
