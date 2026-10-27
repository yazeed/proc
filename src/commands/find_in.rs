//! `proc in` - Filter processes by working directory
//!
//! Examples:
//!   proc in .                  # Processes in current directory
//!   proc in /path/to/project   # Processes in specific directory
//!   proc in . --by node        # Node processes in cwd
//!   proc in ~/projects         # Processes in ~/projects

use crate::core::{sort_processes, Process, SortKey, StatusFilter};
use crate::error::{ProcError, Result};
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

    /// Only show processes using more than this CPU %
    #[arg(long)]
    pub min_cpu: Option<f32>,

    /// Only show processes using more than this memory (MB)
    #[arg(long)]
    pub min_mem: Option<f64>,

    /// Filter by status: running, sleeping, stopped, zombie
    #[arg(long, value_enum, ignore_case = true)]
    pub status: Option<StatusFilter>,

    /// Only show processes running longer than this (seconds)
    #[arg(long)]
    pub min_uptime: Option<u64>,

    /// Only show children of this parent PID
    #[arg(long)]
    pub parent: Option<u32>,

    /// Output as JSON
    #[arg(long, short = 'j')]
    pub json: bool,

    /// Show verbose output with command line, cwd, parent PID, and uptime
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Print full directories and arguments instead of fitting the terminal width
    #[arg(long, short = 'w')]
    pub wide: bool,

    /// Limit the number of results
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,

    /// Sort by: cpu, mem, pid, name
    #[arg(long, short = 's', value_enum, default_value_t = SortKey::Cpu)]
    pub sort: SortKey,
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
        let printer = Printer::new(format, self.verbose).wide(self.wide);

        // Get base process list
        let mut processes = if let Some(ref name) = self.by_name {
            Process::find_by_name(name)?
        } else {
            Process::find_all()?
        };

        // Real CPU readings for the table, --min-cpu and --sort cpu
        Process::measure_cpu(&mut processes);

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

        // Directory filter (required for this command)
        processes.retain(|p| {
            p.cwd
                .as_ref()
                .is_some_and(|cwd| PathBuf::from(cwd).starts_with(&dir_filter))
        });

        // A missing path with nothing running there is a typo, not an empty
        // directory (same as `proc for`). A deleted directory that processes
        // still run in is kept, so its leftovers can be found.
        if processes.is_empty() && !dir_filter.is_dir() {
            return Err(ProcError::InvalidInput(format!(
                "Directory not found: {}",
                self.path
            )));
        }

        // Apply filters
        processes.retain(|p| {
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
            if let Some(status) = self.status {
                if !status.matches(p.status) {
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
        sort_processes(&mut processes, self.sort);

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

        printer.print_processes_as("in", &processes, context.as_deref());
        Ok(())
    }
}
