//! `proc ports` - List all listening ports
//!
//! Examples:
//!   proc ports              # Show all listening ports
//!   proc ports --filter node # Filter by process name
//!   proc ports --exposed    # Only network-accessible ports (0.0.0.0)
//!   proc ports --local      # Only localhost ports (127.0.0.1)
//!   proc ports -v           # Show with executable paths

use crate::core::{PortInfo, Process};
use crate::error::Result;
use crate::ui::{OutputFormat, Printer};
use clap::Args;
use colored::*;
use serde::Serialize;
use std::collections::HashMap;

/// List all listening ports
#[derive(Args, Debug)]
pub struct PortsCommand {
    /// Filter by process name
    #[arg(long, short = 'f')]
    pub filter: Option<String>,

    /// Only show network-exposed ports (0.0.0.0, ::)
    #[arg(long, short = 'e')]
    pub exposed: bool,

    /// Only show localhost ports (127.0.0.1, ::1)
    #[arg(long, short = 'l')]
    pub local: bool,

    /// Output as JSON
    #[arg(long, short = 'j')]
    pub json: bool,

    /// Show verbose output (includes executable path)
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Sort by: port, pid, name
    #[arg(long, short = 's', default_value = "port")]
    pub sort: String,
}

impl PortsCommand {
    pub fn execute(&self) -> Result<()> {
        let mut ports = PortInfo::get_all_listening()?;

        // Filter by process name if specified
        if let Some(ref filter) = self.filter {
            let filter_lower = filter.to_lowercase();
            ports.retain(|p| p.process_name.to_lowercase().contains(&filter_lower));
        }

        // Filter by address exposure
        if self.exposed {
            ports.retain(|p| {
                p.address
                    .as_ref()
                    .map(|a| a == "0.0.0.0" || a == "::" || a == "*")
                    .unwrap_or(true)
            });
        }

        if self.local {
            ports.retain(|p| {
                p.address
                    .as_ref()
                    .map(|a| a == "127.0.0.1" || a == "::1" || a.starts_with("[::1]"))
                    .unwrap_or(false)
            });
        }

        // Sort ports
        match self.sort.to_lowercase().as_str() {
            "port" => ports.sort_by_key(|p| p.port),
            "pid" => ports.sort_by_key(|p| p.pid),
            "name" => ports.sort_by(|a, b| {
                a.process_name
                    .to_lowercase()
                    .cmp(&b.process_name.to_lowercase())
            }),
            _ => ports.sort_by_key(|p| p.port),
        }

        // In verbose mode, fetch process info for paths
        let process_map: HashMap<u32, Process> = if self.verbose {
            let mut map = HashMap::new();
            for port in &ports {
                if !map.contains_key(&port.pid) {
                    if let Ok(Some(proc)) = Process::find_by_pid(port.pid) {
                        map.insert(port.pid, proc);
                    }
                }
            }
            map
        } else {
            HashMap::new()
        };

        if self.json {
            self.print_json(&ports, &process_map);
        } else {
            self.print_human(&ports, &process_map);
        }

        Ok(())
    }

    fn print_human(&self, ports: &[PortInfo], process_map: &HashMap<u32, Process>) {
        if ports.is_empty() {
            println!("{} No listening ports found", "⚠".yellow().bold());
            return;
        }

        println!(
            "{} Found {} listening port{}",
            "✓".green().bold(),
            ports.len().to_string().cyan().bold(),
            if ports.len() == 1 { "" } else { "s" }
        );
        println!();

        // Header
        println!(
            "{:<8} {:<10} {:<8} {:<20} {:<15}",
            "PORT".bright_blue().bold(),
            "PROTO".bright_blue().bold(),
            "PID".bright_blue().bold(),
            "PROCESS".bright_blue().bold(),
            "ADDRESS".bright_blue().bold()
        );
        println!("{}", "─".repeat(65).bright_black());

        for port in ports {
            let addr = port.address.as_deref().unwrap_or("*");
            let proto = format!("{:?}", port.protocol).to_uppercase();

            println!(
                "{:<8} {:<10} {:<8} {:<20} {:<15}",
                port.port.to_string().cyan().bold(),
                proto.white(),
                port.pid.to_string().cyan(),
                truncate_string(&port.process_name, 19).white(),
                addr.bright_black()
            );

            // In verbose mode, show path
            if self.verbose {
                if let Some(proc) = process_map.get(&port.pid) {
                    if let Some(ref path) = proc.exe_path {
                        println!(
                            "         {} {}",
                            "↳".bright_black(),
                            truncate_string(path, 55).bright_black()
                        );
                    }
                }
            }
        }
        println!();
    }

    fn print_json(&self, ports: &[PortInfo], process_map: &HashMap<u32, Process>) {
        let printer = Printer::new(OutputFormat::Json, self.verbose);

        #[derive(Serialize)]
        struct PortWithProcess<'a> {
            #[serde(flatten)]
            port: &'a PortInfo,
            #[serde(skip_serializing_if = "Option::is_none")]
            exe_path: Option<&'a str>,
        }

        let enriched: Vec<PortWithProcess> = ports
            .iter()
            .map(|p| PortWithProcess {
                port: p,
                exe_path: process_map
                    .get(&p.pid)
                    .and_then(|proc| proc.exe_path.as_deref()),
            })
            .collect();

        #[derive(Serialize)]
        struct Output<'a> {
            action: &'static str,
            success: bool,
            count: usize,
            ports: Vec<PortWithProcess<'a>>,
        }

        printer.print_json(&Output {
            action: "ports",
            success: true,
            count: ports.len(),
            ports: enriched,
        });
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
