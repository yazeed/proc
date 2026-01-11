//! Output formatting for proc CLI
//!
//! Provides colored terminal output and JSON formatting.

use crate::core::{PortInfo, Process};
use colored::*;
use serde::Serialize;

/// Output format selection
#[derive(Debug, Clone, Copy, Default)]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
}

/// Main printer for CLI output
pub struct Printer {
    format: OutputFormat,
    verbose: bool,
}

impl Printer {
    pub fn new(format: OutputFormat, verbose: bool) -> Self {
        Self { format, verbose }
    }

    /// Print a success message
    pub fn success(&self, message: &str) {
        match self.format {
            OutputFormat::Human => {
                println!("{} {}", "✓".green().bold(), message.green());
            }
            OutputFormat::Json => {
                // JSON output handled separately
            }
        }
    }

    /// Print an error message
    pub fn error(&self, message: &str) {
        match self.format {
            OutputFormat::Human => {
                eprintln!("{} {}", "✗".red().bold(), message.red());
            }
            OutputFormat::Json => {
                // JSON output handled separately
            }
        }
    }

    /// Print a warning message
    pub fn warning(&self, message: &str) {
        match self.format {
            OutputFormat::Human => {
                println!("{} {}", "⚠".yellow().bold(), message.yellow());
            }
            OutputFormat::Json => {
                // JSON output handled separately
            }
        }
    }

    /// Print a list of processes
    pub fn print_processes(&self, processes: &[Process]) {
        match self.format {
            OutputFormat::Human => self.print_processes_human(processes),
            OutputFormat::Json => self.print_json(&ProcessListOutput {
                action: "find",
                success: true,
                count: processes.len(),
                processes,
            }),
        }
    }

    fn print_processes_human(&self, processes: &[Process]) {
        if processes.is_empty() {
            self.warning("No processes found");
            return;
        }

        println!(
            "{} Found {} process{}",
            "✓".green().bold(),
            processes.len().to_string().cyan().bold(),
            if processes.len() == 1 { "" } else { "es" }
        );
        println!();

        // Header
        println!(
            "{:<8} {:<25} {:>8} {:>10} {:>10}",
            "PID".bright_blue().bold(),
            "NAME".bright_blue().bold(),
            "CPU%".bright_blue().bold(),
            "MEM (MB)".bright_blue().bold(),
            "STATUS".bright_blue().bold()
        );
        println!("{}", "─".repeat(65).bright_black());

        for proc in processes {
            let name = truncate_string(&proc.name, 24);
            let status_str = format!("{:?}", proc.status);
            let status_colored = match proc.status {
                crate::core::ProcessStatus::Running => status_str.green(),
                crate::core::ProcessStatus::Sleeping => status_str.blue(),
                crate::core::ProcessStatus::Stopped => status_str.yellow(),
                crate::core::ProcessStatus::Zombie => status_str.red(),
                _ => status_str.white(),
            };

            println!(
                "{:<8} {:<25} {:>8.1} {:>10.1} {:>10}",
                proc.pid.to_string().cyan(),
                name.white(),
                proc.cpu_percent,
                proc.memory_mb,
                status_colored
            );

            if self.verbose {
                if let Some(ref cmd) = proc.command {
                    let cmd_display = truncate_string(cmd, 60);
                    println!(
                        "         {} {}",
                        "cmd:".bright_black(),
                        cmd_display.bright_black()
                    );
                }
                if let Some(ppid) = proc.parent_pid {
                    println!(
                        "         {} {}",
                        "parent:".bright_black(),
                        ppid.to_string().bright_black()
                    );
                }
            }
        }
        println!();
    }

    /// Print port information
    pub fn print_ports(&self, ports: &[PortInfo]) {
        match self.format {
            OutputFormat::Human => self.print_ports_human(ports),
            OutputFormat::Json => self.print_json(&PortListOutput {
                action: "ports",
                success: true,
                count: ports.len(),
                ports,
            }),
        }
    }

    fn print_ports_human(&self, ports: &[PortInfo]) {
        if ports.is_empty() {
            self.warning("No listening ports found");
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
        }
        println!();
    }

    /// Print a single port info (for `proc on :port`)
    pub fn print_port_info(&self, port_info: &PortInfo) {
        match self.format {
            OutputFormat::Human => {
                println!(
                    "{} Process on port {}:",
                    "✓".green().bold(),
                    port_info.port.to_string().cyan().bold()
                );
                println!();
                println!(
                    "  {} {}",
                    "Name:".bright_black(),
                    port_info.process_name.white().bold()
                );
                println!(
                    "  {} {}",
                    "PID:".bright_black(),
                    port_info.pid.to_string().cyan()
                );
                println!("  {} {:?}", "Protocol:".bright_black(), port_info.protocol);
                if let Some(ref addr) = port_info.address {
                    println!("  {} {}", "Address:".bright_black(), addr);
                }
                println!();
            }
            OutputFormat::Json => self.print_json(&SinglePortOutput {
                action: "on",
                success: true,
                port: port_info,
            }),
        }
    }

    /// Print JSON output for any serializable type
    pub fn print_json<T: Serialize>(&self, data: &T) {
        match serde_json::to_string_pretty(data) {
            Ok(json) => println!("{}", json),
            Err(e) => eprintln!("Failed to serialize JSON: {}", e),
        }
    }

    /// Print kill confirmation
    pub fn print_kill_result(&self, killed: &[Process], failed: &[(Process, String)]) {
        match self.format {
            OutputFormat::Human => {
                if !killed.is_empty() {
                    println!(
                        "{} Killed {} process{}",
                        "✓".green().bold(),
                        killed.len().to_string().cyan().bold(),
                        if killed.len() == 1 { "" } else { "es" }
                    );
                    for proc in killed {
                        println!(
                            "  {} {} [PID {}]",
                            "→".bright_black(),
                            proc.name.white(),
                            proc.pid.to_string().cyan()
                        );
                    }
                }
                if !failed.is_empty() {
                    println!(
                        "{} Failed to kill {} process{}",
                        "✗".red().bold(),
                        failed.len(),
                        if failed.len() == 1 { "" } else { "es" }
                    );
                    for (proc, err) in failed {
                        println!(
                            "  {} {} [PID {}]: {}",
                            "→".bright_black(),
                            proc.name.white(),
                            proc.pid.to_string().cyan(),
                            err.red()
                        );
                    }
                }
            }
            OutputFormat::Json => {
                self.print_json(&KillOutput {
                    action: "kill",
                    success: failed.is_empty(),
                    killed_count: killed.len(),
                    failed_count: failed.len(),
                    killed,
                    failed: &failed
                        .iter()
                        .map(|(p, e)| FailedKill {
                            process: p,
                            error: e,
                        })
                        .collect::<Vec<_>>(),
                });
            }
        }
    }
}

/// Truncate a string to a maximum length
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

// JSON output structures
#[derive(Serialize)]
struct ProcessListOutput<'a> {
    action: &'static str,
    success: bool,
    count: usize,
    processes: &'a [Process],
}

#[derive(Serialize)]
struct PortListOutput<'a> {
    action: &'static str,
    success: bool,
    count: usize,
    ports: &'a [PortInfo],
}

#[derive(Serialize)]
struct SinglePortOutput<'a> {
    action: &'static str,
    success: bool,
    port: &'a PortInfo,
}

#[derive(Serialize)]
struct KillOutput<'a> {
    action: &'static str,
    success: bool,
    killed_count: usize,
    failed_count: usize,
    killed: &'a [Process],
    failed: &'a [FailedKill<'a>],
}

#[derive(Serialize)]
struct FailedKill<'a> {
    process: &'a Process,
    error: &'a str,
}

impl Default for Printer {
    fn default() -> Self {
        Self::new(OutputFormat::Human, false)
    }
}
