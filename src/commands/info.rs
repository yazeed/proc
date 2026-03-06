//! Info command - Get detailed process information
//!
//! Usage:
//!   proc info 1234              # Info for PID
//!   proc info :3000             # Info for process on port 3000
//!   proc info node              # Info for processes named node
//!   proc info :3000,:8080       # Info for multiple targets
//!   proc info :3000,1234,node   # Mixed targets (port + PID + name)

use crate::core::{parse_targets, resolve_in_dir, resolve_target, Process};
use crate::error::Result;
use crate::ui::{colorize_status, format_duration, format_memory, OutputFormat, Printer};
use clap::Args;
use colored::*;
use serde::Serialize;
use std::path::PathBuf;

/// Show detailed process information
#[derive(Args, Debug)]
pub struct InfoCommand {
    /// Target(s): PID, :port, or name (comma-separated for multiple)
    #[arg(required = true)]
    targets: Vec<String>,

    /// Output as JSON
    #[arg(long, short = 'j')]
    json: bool,

    /// Show extra details
    #[arg(long, short = 'v')]
    verbose: bool,

    /// Filter by directory (defaults to current directory if no path given)
    #[arg(long = "in", short = 'i', num_args = 0..=1, default_missing_value = ".")]
    pub in_dir: Option<String>,

    /// Filter by process name
    #[arg(long = "by", short = 'b')]
    pub by_name: Option<String>,
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

        // Apply --in and --by filters
        let in_dir_filter = resolve_in_dir(&self.in_dir);
        found.retain(|p| {
            if let Some(ref dir_path) = in_dir_filter {
                if let Some(ref cwd) = p.cwd {
                    if !PathBuf::from(cwd).starts_with(dir_path) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            if let Some(ref name) = self.by_name {
                if !p.name.to_lowercase().contains(&name.to_lowercase()) {
                    return false;
                }
            }
            true
        });

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
                printer.warning(&format!("Not found: {}", not_found.join(", ")));
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

        if let Some(ref cwd) = proc.cwd {
            println!("  {} {}", "Directory:".bright_black(), cwd);
        }

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
        let status_colored = colorize_status(&proc.status, &status_str);
        println!("  {} {}", "Status:".bright_black(), status_colored);

        println!("  {} {:.1}%", "CPU:".bright_black(), proc.cpu_percent);
        println!(
            "  {} {}",
            "Memory:".bright_black(),
            format_memory(proc.memory_mb)
        );

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

#[derive(Serialize)]
struct InfoOutput<'a> {
    action: &'static str,
    success: bool,
    found_count: usize,
    not_found_count: usize,
    processes: &'a [Process],
    not_found: &'a [String],
}
