//! Info command - Get detailed process information
//!
//! Usage:
//!   proc info 1234              # Info for PID
//!   proc info :3000             # Info for process on port 3000
//!   proc info node              # Info for processes named node
//!   proc info :3000,:8080       # Info for multiple targets
//!   proc info :3000,1234,node   # Mixed targets (port + PID + name)

use crate::core::{parse_targets, resolve_target, Process, ProcessStatus};
use crate::error::Result;
use crate::ui::{OutputFormat, Printer};
use clap::Args;
use colored::*;
use serde::Serialize;

/// Show detailed process information
#[derive(Args, Debug)]
pub struct InfoCommand {
    /// Target(s): PID, :port, or name (comma-separated for multiple)
    #[arg(required = true)]
    targets: Vec<String>,

    /// Output as JSON
    #[arg(long, short)]
    json: bool,

    /// Show extra details
    #[arg(long, short)]
    verbose: bool,
}

impl InfoCommand {
    /// Executes the info command, displaying detailed process information.
    pub fn execute(&self) -> Result<()> {
        let format = if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };
        let printer = Printer::new(format, self.verbose);

        // Flatten targets - support both space-separated and comma-separated
        let all_targets: Vec<String> = self.targets.iter().flat_map(|t| parse_targets(t)).collect();

        let mut found = Vec::new();
        let mut not_found = Vec::new();
        let mut seen_pids = std::collections::HashSet::new();

        for target in &all_targets {
            match resolve_target(target) {
                Ok(processes) => {
                    if processes.is_empty() {
                        not_found.push(target.clone());
                    } else {
                        for proc in processes {
                            // Deduplicate by PID
                            if seen_pids.insert(proc.pid) {
                                found.push(proc);
                            }
                        }
                    }
                }
                Err(_) => not_found.push(target.clone()),
            }
        }

        if self.json {
            printer.print_json(&InfoOutput {
                action: "info",
                success: !found.is_empty(),
                found_count: found.len(),
                not_found_count: not_found.len(),
                processes: &found,
                not_found: &not_found,
            });
        } else {
            for proc in &found {
                self.print_process_info(proc);
            }

            if !not_found.is_empty() {
                for target in &not_found {
                    printer.warning(&format!("Target '{}' not found", target));
                }
            }
        }

        Ok(())
    }

    fn print_process_info(&self, proc: &Process) {
        println!(
            "{} Process {}",
            "✓".green().bold(),
            proc.pid.to_string().cyan().bold()
        );
        println!();
        println!("  {} {}", "Name:".bright_black(), proc.name.white().bold());
        println!(
            "  {} {}",
            "PID:".bright_black(),
            proc.pid.to_string().cyan()
        );

        if let Some(ref path) = proc.exe_path {
            println!("  {} {}", "Path:".bright_black(), path);
        }

        if let Some(ref user) = proc.user {
            println!("  {} {}", "User:".bright_black(), user);
        }

        if let Some(ppid) = proc.parent_pid {
            println!(
                "  {} {}",
                "Parent PID:".bright_black(),
                ppid.to_string().cyan()
            );
        }

        let status_str = format!("{:?}", proc.status);
        let status_colored = match proc.status {
            ProcessStatus::Running => status_str.green(),
            ProcessStatus::Sleeping => status_str.blue(),
            ProcessStatus::Stopped => status_str.yellow(),
            ProcessStatus::Zombie => status_str.red(),
            _ => status_str.white(),
        };
        println!("  {} {}", "Status:".bright_black(), status_colored);

        println!("  {} {:.1}%", "CPU:".bright_black(), proc.cpu_percent);
        println!("  {} {:.1} MB", "Memory:".bright_black(), proc.memory_mb);

        if let Some(start_time) = proc.start_time {
            let duration = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs().saturating_sub(start_time))
                .unwrap_or(0);

            let uptime = format_duration(duration);
            println!("  {} {}", "Uptime:".bright_black(), uptime);
        }

        if self.verbose {
            if let Some(ref cmd) = proc.command {
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
struct InfoOutput<'a> {
    action: &'static str,
    success: bool,
    found_count: usize,
    not_found_count: usize,
    processes: &'a [Process],
    not_found: &'a [String],
}
