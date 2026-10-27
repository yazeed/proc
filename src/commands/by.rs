//! `proc by` - Filter processes by name
//!
//! Examples:
//!   proc by node               # Processes named 'node'
//!   proc by node --in .        # Node processes in current directory
//!   proc by node --min-cpu 5   # Node processes using >5% CPU
//!   proc by "my app"           # Processes with spaces in name

use crate::core::{resolve_in_dir, sort_processes, Process, SortKey, StatusFilter};
use crate::error::Result;
use crate::ui::Printer;
use clap::Args;
use std::path::PathBuf;

/// Filter processes by name
#[derive(Args, Debug)]
pub struct ByCommand {
    /// Process name or pattern to match
    pub name: String,

    /// Filter by directory (defaults to current directory if no path given)
    #[arg(long = "in", short = 'i', num_args = 0..=1, default_missing_value = ".")]
    pub in_dir: Option<String>,

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

impl ByCommand {
    /// Executes the by command, listing processes matching the name filter.
    pub fn execute(&self) -> Result<()> {
        let printer = Printer::from_flags(self.json, self.verbose).wide(self.wide);

        // Get processes by name
        let mut processes = Process::find_by_name(&self.name)?;

        // Resolve --in filter path
        let in_dir_filter = resolve_in_dir(&self.in_dir);

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
        let mut context_parts = vec![format!("by '{}'", self.name)];
        if let Some(ref dir) = in_dir_filter {
            context_parts.push(format!("in {}", dir.display()));
        }
        let context = Some(context_parts.join(" "));

        printer.print_processes_as("by", &processes, context.as_deref());
        Ok(())
    }
}
