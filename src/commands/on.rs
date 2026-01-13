//! `proc on` - Port/process lookup
//!
//! Usage:
//!   proc on :3000      # What process is on port 3000?
//!   proc on 1234       # What ports is PID 1234 listening on?
//!   proc on node       # What ports are node processes listening on?

use crate::core::{
    find_ports_for_pid, parse_target, resolve_target, PortInfo, Process, TargetType,
};
use crate::error::{ProcError, Result};
use clap::Args;
use colored::*;
use serde::Serialize;

/// Show what's on a port, or what ports a process is on
#[derive(Args, Debug)]
pub struct OnCommand {
    /// Target: :port, PID, or process name
    pub target: String,

    /// Output as JSON
    #[arg(long, short = 'j')]
    pub json: bool,

    /// Show verbose output (full command line)
    #[arg(long, short = 'v')]
    pub verbose: bool,
}

impl OnCommand {
    pub fn execute(&self) -> Result<()> {
        match parse_target(&self.target) {
            TargetType::Port(port) => self.show_process_on_port(port),
            TargetType::Pid(pid) => self.show_ports_for_pid(pid),
            TargetType::Name(name) => self.show_ports_for_name(&name),
        }
    }

    /// Show what process is on a specific port
    fn show_process_on_port(&self, port: u16) -> Result<()> {
        let port_info = match PortInfo::find_by_port(port)? {
            Some(info) => info,
            None => return Err(ProcError::PortNotFound(port)),
        };

        let process = Process::find_by_pid(port_info.pid)?;

        if self.json {
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
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            self.print_process_on_port(&port_info, process.as_ref());
        }

        Ok(())
    }

    /// Show what ports a PID is listening on
    fn show_ports_for_pid(&self, pid: u32) -> Result<()> {
        let process = Process::find_by_pid(pid)?
            .ok_or_else(|| ProcError::ProcessNotFound(pid.to_string()))?;

        let ports = find_ports_for_pid(pid)?;

        if self.json {
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
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            self.print_ports_for_process(&process, &ports);
        }

        Ok(())
    }

    /// Show what ports processes with a given name are listening on
    fn show_ports_for_name(&self, name: &str) -> Result<()> {
        let processes = resolve_target(name)?;

        if processes.is_empty() {
            return Err(ProcError::ProcessNotFound(name.to_string()));
        }

        let mut all_results: Vec<(Process, Vec<PortInfo>)> = Vec::new();

        for proc in processes {
            let ports = find_ports_for_pid(proc.pid)?;
            all_results.push((proc, ports));
        }

        if self.json {
            let output: Vec<_> = all_results
                .iter()
                .map(|(proc, ports)| ProcessPortsJson {
                    process: proc,
                    ports,
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
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
                "  {} {:.1}% CPU, {:.1} MB",
                "Resources:".bright_black(),
                proc.cpu_percent,
                proc.memory_mb
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

        if self.verbose {
            if let Some(ref path) = process.exe_path {
                println!();
                println!("  {} {}", "Path:".bright_black(), path.bright_black());
            }
            if let Some(ref cmd) = process.command {
                println!("  {} {}", "Command:".bright_black(), cmd.bright_black());
            }
        }

        println!();
    }
}

fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86400, (secs % 86400) / 3600)
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
