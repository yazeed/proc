//! `proc why` - Explain why a port is busy or trace process ancestry
//!
//! Examples:
//!   proc why :3000              # Why is port 3000 busy? (ancestry + port context)
//!   proc why node               # Show ancestry for node processes
//!   proc why 1234               # Show ancestry for PID 1234
//!   proc why :3000 --json       # JSON output with port context

use crate::core::{
    find_ports_for_pid, parse_target, parse_targets, resolve_target, PortInfo, Process,
    ProcessStatus, TargetType,
};
use crate::error::Result;
use crate::ui::{OutputFormat, Printer};
use clap::Args;
use colored::*;
use serde::Serialize;
use std::collections::HashMap;

/// Trace why a port is busy or show process ancestry
#[derive(Args, Debug)]
pub struct WhyCommand {
    /// Target(s): :port, PID, or process name (comma-separated for multiple)
    #[arg(required = true)]
    pub target: String,

    /// Output as JSON
    #[arg(long, short = 'j')]
    pub json: bool,

    /// Show verbose output
    #[arg(long, short = 'v')]
    pub verbose: bool,
}

impl WhyCommand {
    /// Executes the why command, showing ancestry and port context.
    pub fn execute(&self) -> Result<()> {
        let format = if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };
        let printer = Printer::new(format, self.verbose);

        let all_processes = Process::find_all()?;
        let pid_map: HashMap<u32, &Process> = all_processes.iter().map(|p| (p.pid, p)).collect();

        let targets = parse_targets(&self.target);

        for (i, target_str) in targets.iter().enumerate() {
            if i > 0 && !self.json {
                println!();
            }

            let parsed = parse_target(target_str);

            // Collect port context for port targets
            let port_context: Option<PortInfo> = if let TargetType::Port(port) = &parsed {
                PortInfo::find_by_port(*port).ok().flatten()
            } else {
                None
            };

            // Resolve to processes
            let target_processes = match resolve_target(target_str) {
                Ok(procs) => procs,
                Err(e) => {
                    if !self.json {
                        printer.warning(&format!("{}", e));
                    }
                    continue;
                }
            };

            if target_processes.is_empty() {
                printer.warning(&format!("No process found for '{}'", target_str));
                continue;
            }

            if self.json {
                let output: Vec<WhyOutput> = target_processes
                    .iter()
                    .map(|proc| {
                        let chain = self.build_ancestry_chain(proc, &pid_map);
                        let ports = find_ports_for_pid(proc.pid).unwrap_or_default();
                        WhyOutput {
                            target: target_str.clone(),
                            port: port_context.as_ref().map(|p| p.port),
                            protocol: port_context
                                .as_ref()
                                .map(|p| format!("{:?}", p.protocol).to_uppercase()),
                            process: WhyProcessInfo {
                                pid: proc.pid,
                                name: proc.name.clone(),
                                command: proc.command.clone(),
                                cwd: proc.cwd.clone(),
                                status: format!("{:?}", proc.status),
                            },
                            ports,
                            ancestry: chain,
                        }
                    })
                    .collect();
                printer.print_json(&output);
            } else {
                // Print port header if applicable
                if let Some(ref port_info) = port_context {
                    println!(
                        "{} Port {} ({}):",
                        "✓".green().bold(),
                        port_info.port.to_string().cyan().bold(),
                        format!("{:?}", port_info.protocol).to_uppercase()
                    );
                } else {
                    println!(
                        "{} Ancestry for '{}':",
                        "✓".green().bold(),
                        target_str.cyan()
                    );
                }
                println!();

                for proc in &target_processes {
                    self.print_ancestry_with_context(proc, &pid_map);
                    println!();
                }
            }
        }

        Ok(())
    }

    /// Build the ancestor chain from root down to target
    fn build_ancestry_chain(
        &self,
        target: &Process,
        pid_map: &HashMap<u32, &Process>,
    ) -> Vec<AncestryEntry> {
        let mut chain: Vec<AncestryEntry> = Vec::new();
        let mut current_pid = Some(target.pid);

        while let Some(pid) = current_pid {
            if let Some(proc) = pid_map.get(&pid) {
                chain.push(AncestryEntry {
                    pid: proc.pid,
                    name: proc.name.clone(),
                    command: proc.command.clone(),
                    cwd: proc.cwd.clone(),
                    status: format!("{:?}", proc.status),
                    is_target: proc.pid == target.pid,
                });
                current_pid = proc.parent_pid;
                if chain.len() > 100 {
                    break;
                }
            } else {
                break;
            }
        }

        chain.reverse();
        chain
    }

    /// Print ancestry tree with working directory and command context
    fn print_ancestry_with_context(&self, target: &Process, pid_map: &HashMap<u32, &Process>) {
        // Build the ancestor chain (from target up to root)
        let mut chain: Vec<&Process> = Vec::new();
        let mut current_pid = Some(target.pid);

        while let Some(pid) = current_pid {
            if let Some(proc) = pid_map.get(&pid) {
                chain.push(proc);
                current_pid = proc.parent_pid;
                if chain.len() > 100 {
                    break;
                }
            } else {
                break;
            }
        }

        // Reverse to print from root to target
        chain.reverse();

        for (i, proc) in chain.iter().enumerate() {
            let is_target = proc.pid == target.pid;
            let indent = "    ".repeat(i);
            let connector = if i == 0 { "" } else { "└── " };

            let status_indicator = match proc.status {
                ProcessStatus::Running => "●".green(),
                ProcessStatus::Sleeping => "○".blue(),
                ProcessStatus::Stopped => "◐".yellow(),
                ProcessStatus::Zombie => "✗".red(),
                _ => "?".white(),
            };

            // Build the command summary (skip exe name, show args)
            let cmd_summary = proc
                .command
                .as_ref()
                .and_then(|c| {
                    let parts: Vec<&str> = c.split_whitespace().collect();
                    if parts.len() > 1 {
                        Some(parts[1..].join(" "))
                    } else {
                        None
                    }
                })
                .unwrap_or_default();

            if is_target {
                let cmd_part = if cmd_summary.is_empty() {
                    String::new()
                } else {
                    format!(" {}", cmd_summary)
                };
                println!(
                    "{}{}{} {} [{}]{}  {}",
                    indent.bright_black(),
                    connector.bright_black(),
                    status_indicator,
                    proc.name.cyan().bold(),
                    proc.pid.to_string().cyan().bold(),
                    cmd_part,
                    "← target".yellow()
                );
                // Show working directory for the target
                if let Some(ref cwd) = proc.cwd {
                    let dir_indent = "    ".repeat(i + 1);
                    println!(
                        "{}{}",
                        dir_indent.bright_black(),
                        format!("dir: {}", cwd).bright_black()
                    );
                }
            } else {
                let cmd_part = if cmd_summary.is_empty() {
                    String::new()
                } else {
                    format!(" {}", cmd_summary)
                };
                println!(
                    "{}{}{} {} [{}]{}",
                    indent.bright_black(),
                    connector.bright_black(),
                    status_indicator,
                    proc.name.white(),
                    proc.pid.to_string().cyan(),
                    cmd_part.bright_black()
                );
            }
        }
    }
}

#[derive(Serialize)]
struct WhyOutput {
    target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    protocol: Option<String>,
    process: WhyProcessInfo,
    ports: Vec<PortInfo>,
    ancestry: Vec<AncestryEntry>,
}

#[derive(Serialize)]
struct WhyProcessInfo {
    pid: u32,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cwd: Option<String>,
    status: String,
}

#[derive(Serialize)]
struct AncestryEntry {
    pid: u32,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cwd: Option<String>,
    status: String,
    is_target: bool,
}
