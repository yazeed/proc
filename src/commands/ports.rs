//! `proc ports` - List all listening ports
//!
//! Examples:
//!   proc ports              # Show all listening ports
//!   proc ports --by node     # Filter by process name
//!   proc ports --exposed    # Only network-accessible ports (0.0.0.0)
//!   proc ports --local      # Only localhost ports (127.0.0.1)
//!   proc ports -v           # Show with executable paths

use crate::core::{resolve_in_dir, PortInfo, PortSortKey, Process};
use crate::error::Result;
use crate::ui::{truncate_string, OutputFormat, Printer};
use clap::Args;
use colored::*;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// List all listening ports
#[derive(Args, Debug)]
pub struct PortsCommand {
    /// Filter by process name
    #[arg(long = "by", short = 'b')]
    pub by_name: Option<String>,

    /// Filter by directory (defaults to current directory if no path given)
    #[arg(long = "in", short = 'i', num_args = 0..=1, default_missing_value = ".")]
    pub in_dir: Option<String>,

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
    #[arg(long, short = 's', value_enum, default_value_t = PortSortKey::Port)]
    pub sort: PortSortKey,

    /// Limit the number of results
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,

    /// Filter ports by range (e.g., 8000-9000)
    #[arg(long, short = 'r')]
    pub range: Option<String>,
}

impl PortsCommand {
    /// Executes the ports command, listing all listening network ports.
    pub fn execute(&self) -> Result<()> {
        let mut ports = PortInfo::get_all_listening()?;

        // Filter by process name if specified
        if let Some(ref name) = self.by_name {
            let name_lower = name.to_lowercase();
            ports.retain(|p| p.process_name.to_lowercase().contains(&name_lower));
        }

        // Filter by directory if specified
        if let Some(ref _dir) = self.in_dir {
            let in_dir_filter = resolve_in_dir(&self.in_dir);
            if let Some(ref dir_path) = in_dir_filter {
                ports.retain(|p| {
                    if let Ok(Some(proc)) = Process::find_by_pid(p.pid) {
                        if let Some(ref cwd) = proc.cwd {
                            return PathBuf::from(cwd).starts_with(dir_path);
                        }
                    }
                    false
                });
            }
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

        // Filter by port range (e.g., "3000-9000")
        if let Some(ref range) = self.range {
            if let Some((start, end)) = range.split_once('-') {
                if let (Ok(start), Ok(end)) =
                    (start.trim().parse::<u16>(), end.trim().parse::<u16>())
                {
                    ports.retain(|p| p.port >= start && p.port <= end);
                }
            }
        }

        // Sort ports
        match self.sort {
            PortSortKey::Port => ports.sort_by_key(|p| p.port),
            PortSortKey::Pid => ports.sort_by_key(|p| p.pid),
            PortSortKey::Name => ports.sort_by(|a, b| {
                a.process_name
                    .to_lowercase()
                    .cmp(&b.process_name.to_lowercase())
            }),
        }

        // Apply limit if specified
        if let Some(limit) = self.limit {
            ports.truncate(limit);
        }

        // In verbose mode, fetch process info for paths
        let process_map: HashMap<u32, Process> = if self.verbose {
            let mut map = HashMap::new();
            for port in &ports {
                if let std::collections::hash_map::Entry::Vacant(e) = map.entry(port.pid) {
                    if let Ok(Some(proc)) = Process::find_by_pid(port.pid) {
                        e.insert(proc);
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
