//! `proc in` - Filter processes by working directory
//!
//! Examples:
//!   proc in .                  # Processes in current directory
//!   proc in /path/to/project   # Processes in specific directory
//!   proc in . --by node        # Node processes in cwd
//!   proc in ~/projects         # Processes in ~/projects

use crate::core::{Process, ProcessStatus};
use crate::error::Result;
use crate::ui::{OutputFormat, Printer};
use clap::Args;
use std::path::PathBuf;

/// Filter processes by working directory
#[derive(Args, Debug)]
pub struct InCommand {
    /// Directory path (absolute, relative, or . for cwd)
    pub path: String,

    /// Filter by process name
    #[arg(long = "by", short = 'b')]
    pub by_name: Option<String>,

    /// Filter by executable path
    #[arg(long, short = 'p')]
    pub exe_path: Option<String>,

    /// Only show processes using more than this CPU %
    #[arg(long)]
    pub min_cpu: Option<f32>,

    /// Only show processes using more than this memory (MB)
    #[arg(long)]
    pub min_mem: Option<f64>,

    /// Filter by status: running, sleeping, stopped, zombie
    #[arg(long)]
    pub status: Option<String>,

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

impl InCommand {
    /// Expand ~ to home directory
    fn expand_tilde(path: &str) -> PathBuf {
        if let Some(stripped) = path.strip_prefix("~/") {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home).join(stripped);
            }
        } else if path == "~" {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home);
            }
        }
        PathBuf::from(path)
    }

    /// Executes the in command, listing processes in the specified directory.
    pub fn execute(&self) -> Result<()> {
        let format = if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };
        let printer = Printer::new(format, self.verbose);

        // Get base process list
        let mut processes = if let Some(ref name) = self.by_name {
            Process::find_by_name(name)?
        } else {
            Process::find_all()?
        };

        // Resolve directory path
        let dir_filter = if self.path == "." {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        } else {
            let expanded = Self::expand_tilde(&self.path);
            if expanded.is_relative() {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(expanded)
            } else {
                expanded
            }
        };

        // Resolve executable path filter
        let exe_path_filter: Option<PathBuf> = self.exe_path.as_ref().map(|p| {
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
            // Directory filter (required for this command)
            if let Some(ref proc_cwd) = p.cwd {
                let proc_path = PathBuf::from(proc_cwd);
                if !proc_path.starts_with(&dir_filter) {
                    return false;
                }
            } else {
                return false;
            }

            // Executable path filter
            if let Some(ref exe_path) = exe_path_filter {
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

        // Build context string for output
        let mut context_parts = vec![format!("in {}", dir_filter.display())];
        if let Some(ref name) = self.by_name {
            context_parts.push(format!("by '{}'", name));
        }
        let context = Some(context_parts.join(" "));

        printer.print_processes_with_context(&processes, context.as_deref());
        Ok(())
    }
}
