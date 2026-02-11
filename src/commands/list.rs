//! `proc list` - List processes
//!
//! Examples:
//!   proc list                  # List all processes
//!   proc list node             # Filter by name
//!   proc list --in             # Processes in current directory
//!   proc list --in /project    # Processes in /project
//!   proc list --min-cpu 10     # Processes using >10% CPU

use crate::core::{resolve_in_dir, Process, ProcessStatus};
use crate::error::Result;
use crate::ui::{OutputFormat, Printer};
use clap::Args;
use std::path::PathBuf;

/// List processes
#[derive(Args, Debug)]
pub struct ListCommand {
    /// Process name or pattern to filter by
    pub name: Option<String>,

    /// Filter by directory (defaults to current directory if no path given)
    #[arg(long = "in", short = 'i', num_args = 0..=1, default_missing_value = ".")]
    pub in_dir: Option<String>,

    /// Filter by executable path
    #[arg(long, short = 'p')]
    pub path: Option<String>,

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

    /// Only show children of this parent PID
    #[arg(long)]
    pub parent: Option<u32>,

    /// Output as JSON
    #[arg(long, short = 'j')]
    pub json: bool,

    /// Show verbose output with command line, cwd, and parent PID
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Limit the number of results
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,

    /// Sort by: cpu, mem, pid, name
    #[arg(long, short = 's', default_value = "cpu")]
    pub sort: String,
}

impl ListCommand {
    /// Executes the list command, displaying processes matching the filters.
    pub fn execute(&self) -> Result<()> {
        let format = if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };
        let printer = Printer::new(format, self.verbose);

        // Get base process list (supports comma-separated names)
        let mut processes = if let Some(ref name) = self.name {
            let names: Vec<String> = name
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let mut all = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for n in &names {
                if let Ok(found) = Process::find_by_name(n) {
                    for p in found {
                        if seen.insert(p.pid) {
                            all.push(p);
                        }
                    }
                }
            }
            all
        } else {
            Process::find_all()?
        };

        // Resolve --in filter path
        let in_dir_filter = resolve_in_dir(&self.in_dir);

        // Resolve path filter
        let path_filter: Option<PathBuf> = self.path.as_ref().map(|p| {
            let path = PathBuf::from(p);
            if path.is_relative() {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(path)
            } else {
                path
            }
        });

        // Apply filters
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

            // Path filter (executable path)
            if let Some(ref exe_path) = path_filter {
                if let Some(ref proc_exe) = p.exe_path {
                    let proc_path = PathBuf::from(proc_exe);
                    if !proc_path.starts_with(exe_path) {
                        return false;
                    }
                } else {
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

            // Parent PID filter
            if let Some(ppid) = self.parent {
                if p.parent_pid != Some(ppid) {
                    return false;
                }
            }

            true
        });

        // Sort processes
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
            _ => {} // Keep default order
        }

        // Apply limit if specified
        if let Some(limit) = self.limit {
            processes.truncate(limit);
        }

        // Build context string for output (e.g., "in /path/to/dir")
        let context = in_dir_filter
            .as_ref()
            .map(|p| format!("in {}", p.display()));

        printer.print_processes_with_context(&processes, context.as_deref());
        Ok(())
    }
}
