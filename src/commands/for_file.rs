//! `proc for` - Find processes by file path
//!
//! Examples:
//!   proc for script.py           # What's running this script?
//!   proc for ./app               # Relative path
//!   proc for /var/log/app.log    # What has this file open?
//!   proc for ~/bin/myapp         # Tilde expansion

use crate::core::{find_ports_for_pid, resolve_in_dir, PortInfo, Process, ProcessStatus};
use crate::error::{ProcError, Result};
use crate::ui::truncate_string;
use clap::Args;
use colored::*;
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;

/// Find processes by file path
#[derive(Args, Debug)]
pub struct ForCommand {
    /// File path (relative, absolute, or with ~)
    pub file: String,

    /// Filter by working directory (defaults to current directory if no path given)
    #[arg(long = "in", short = 'i', num_args = 0..=1, default_missing_value = ".")]
    pub in_dir: Option<String>,

    /// Filter by process name
    #[arg(long = "by", short = 'b')]
    pub by_name: Option<String>,

    /// Only show processes using more than this CPU %
    #[arg(long)]
    pub min_cpu: Option<f32>,

    /// Only show processes using more than this memory (MB)
    #[arg(long)]
    pub min_mem: Option<f64>,

    /// Filter by status: running, sleeping, stopped, zombie
    #[arg(long)]
    pub status: Option<String>,

    /// Only show processes running longer than this (seconds)
    #[arg(long)]
    pub min_uptime: Option<u64>,

    /// Output as JSON
    #[arg(long, short = 'j')]
    pub json: bool,

    /// Show verbose output
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Sort by: cpu, mem, pid, name
    #[arg(long, short = 's', default_value = "cpu")]
    pub sort: String,

    /// Limit the number of results
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,
}

impl ForCommand {
    /// Executes the for command, finding processes by file path.
    pub fn execute(&self) -> Result<()> {
        // 1. Resolve file path (relative, tilde, canonicalize)
        let file_path = self.resolve_path(&self.file)?;

        // 2. Find processes by executable path
        let exe_processes = Process::find_by_exe_path(&file_path)?;

        // 3. Find processes with file open (lsof)
        let open_file_procs = Process::find_by_open_file(&file_path)?;

        // 4. Merge and deduplicate by PID
        let mut seen_pids = HashSet::new();
        let mut processes: Vec<Process> = Vec::new();

        for proc in exe_processes {
            if seen_pids.insert(proc.pid) {
                processes.push(proc);
            }
        }

        for proc in open_file_procs {
            if seen_pids.insert(proc.pid) {
                processes.push(proc);
            }
        }

        // 5. Apply filters
        self.apply_filters(&mut processes);

        // 6. Sort processes
        match self.sort.to_lowercase().as_str() {
            "cpu" => processes.sort_by(|a, b| {
                b.cpu_percent
                    .partial_cmp(&a.cpu_percent)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            "mem" | "memory" => processes.sort_by(|a, b| {
                b.memory_mb
                    .partial_cmp(&a.memory_mb)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            "pid" => processes.sort_by_key(|p| p.pid),
            "name" => processes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
            _ => {}
        }

        // 7. Apply limit if specified
        if let Some(limit) = self.limit {
            processes.truncate(limit);
        }

        // 8. Handle no results
        if processes.is_empty() {
            return Err(ProcError::ProcessNotFound(format!(
                "No processes found for file: {}",
                self.file
            )));
        }

        // 9. For each process, get ports
        let mut results: Vec<(Process, Vec<PortInfo>)> = Vec::new();
        for proc in processes {
            let ports = find_ports_for_pid(proc.pid)?;
            results.push((proc, ports));
        }

        // 10. Output
        if self.json {
            self.print_json(&results)?;
        } else {
            self.print_human(&results);
        }

        Ok(())
    }

    fn resolve_path(&self, path: &str) -> Result<PathBuf> {
        // Tilde expansion
        let expanded = if let Some(stripped) = path.strip_prefix("~/") {
            if let Ok(home) = std::env::var("HOME") {
                PathBuf::from(home).join(stripped)
            } else {
                PathBuf::from(path)
            }
        } else if path == "~" {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
        } else {
            PathBuf::from(path)
        };

        // Make absolute if relative
        let absolute = if expanded.is_relative() {
            std::env::current_dir()?.join(expanded)
        } else {
            expanded
        };

        // Canonicalize (resolve symlinks, normalize)
        absolute
            .canonicalize()
            .map_err(|_| ProcError::InvalidInput(format!("File not found: {}", path)))
    }

    fn apply_filters(&self, processes: &mut Vec<Process>) {
        let in_dir_filter = resolve_in_dir(&self.in_dir);

        processes.retain(|p| {
            // Directory filter (--in)
            if let Some(ref dir_path) = in_dir_filter {
                if let Some(ref proc_cwd) = p.cwd {
                    let proc_path = PathBuf::from(proc_cwd);
                    if !proc_path.starts_with(dir_path) {
                        return false;
                    }
                } else {
                    return false;
                }
            }

            // Name filter (--by)
            if let Some(ref name) = self.by_name {
                let name_lower = name.to_lowercase();
                if !p.name.to_lowercase().contains(&name_lower) {
                    return false;
                }
            }

            // CPU filter
            if let Some(min_cpu) = self.min_cpu {
                if p.cpu_percent < min_cpu {
                    return false;
                }
            }

            // Memory filter
            if let Some(min_mem) = self.min_mem {
                if p.memory_mb < min_mem {
                    return false;
                }
            }

            // Status filter
            if let Some(ref status) = self.status {
                let status_match = match status.to_lowercase().as_str() {
                    "running" => matches!(p.status, ProcessStatus::Running),
                    "sleeping" | "sleep" => matches!(p.status, ProcessStatus::Sleeping),
                    "stopped" | "stop" => matches!(p.status, ProcessStatus::Stopped),
                    "zombie" => matches!(p.status, ProcessStatus::Zombie),
                    _ => true,
                };
                if !status_match {
                    return false;
                }
            }

            // Uptime filter
            if let Some(min_uptime) = self.min_uptime {
                if let Some(start_time) = p.start_time {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    if now.saturating_sub(start_time) < min_uptime {
                        return false;
                    }
                } else {
                    return false;
                }
            }

            true
        });
    }

    fn print_human(&self, results: &[(Process, Vec<PortInfo>)]) {
        let count = results.len();
        let file_display = &self.file;

        if count == 1 {
            // Single process - detailed view (like `on` command)
            let (proc, ports) = &results[0];

            println!(
                "{} Found 1 process for {}",
                "✓".green().bold(),
                file_display.cyan().bold()
            );
            println!();

            println!("  {}", "Process:".bright_black());
            println!(
                "    {} {} (PID {})",
                "Name:".bright_black(),
                proc.name.white().bold(),
                proc.pid.to_string().cyan()
            );
            println!("    {} {:.1}%", "CPU:".bright_black(), proc.cpu_percent);
            println!("    {} {:.1} MB", "MEM:".bright_black(), proc.memory_mb);

            if let Some(ref path) = proc.exe_path {
                println!("    {} {}", "Path:".bright_black(), path.bright_black());
            }

            if self.verbose {
                if let Some(ref cwd) = proc.cwd {
                    println!("    {} {}", "CWD:".bright_black(), cwd.bright_black());
                }
                if let Some(ref cmd) = proc.command {
                    println!("    {} {}", "Command:".bright_black(), cmd.bright_black());
                }
            }

            println!();

            if ports.is_empty() {
                println!("  {} No listening ports", "ℹ".blue());
            } else {
                println!("  {}", "Listening Ports:".bright_black());
                for port_info in ports {
                    let addr = port_info.address.as_deref().unwrap_or("*");
                    println!(
                        "    {} :{} ({} on {})",
                        "→".bright_black(),
                        port_info.port.to_string().cyan(),
                        format!("{:?}", port_info.protocol).to_uppercase(),
                        addr
                    );
                }
            }
        } else {
            // Multiple processes - table view
            println!(
                "{} Found {} processes for {}",
                "✓".green().bold(),
                count.to_string().white().bold(),
                file_display.cyan().bold()
            );
            println!();

            // Header
            println!(
                "  {:>7}  {:<15}  {:>5}  {:>8}  {}",
                "PID".bright_black(),
                "NAME".bright_black(),
                "CPU%".bright_black(),
                "MEM".bright_black(),
                "PORTS".bright_black()
            );
            println!("  {}", "─".repeat(60).bright_black());

            for (proc, ports) in results {
                let ports_str = if ports.is_empty() {
                    "-".to_string()
                } else {
                    ports
                        .iter()
                        .map(|p| format!(":{}", p.port))
                        .collect::<Vec<_>>()
                        .join(", ")
                };

                println!(
                    "  {:>7}  {:<15}  {:>5.1}  {:>6.1}MB  {}",
                    proc.pid.to_string().cyan(),
                    truncate_string(&proc.name, 15).white(),
                    proc.cpu_percent,
                    proc.memory_mb,
                    ports_str
                );
            }
        }

        println!();
    }

    fn print_json(&self, results: &[(Process, Vec<PortInfo>)]) -> Result<()> {
        let output: Vec<ProcessForJson> = results
            .iter()
            .map(|(proc, ports)| ProcessForJson {
                process: proc,
                ports,
            })
            .collect();

        println!("{}", serde_json::to_string_pretty(&output)?);
        Ok(())
    }
}

#[derive(Serialize)]
struct ProcessForJson<'a> {
    process: &'a Process,
    ports: &'a [PortInfo],
}
