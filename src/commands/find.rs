//! `proc find` - Find processes by name
//!
//! Examples:
//!   proc find node          # Find all Node.js processes
//!   proc find python        # Find all Python processes
//!   proc find --all         # List all processes

use crate::core::Process;
use crate::error::Result;
use crate::ui::{OutputFormat, Printer};
use clap::Args;

/// Find processes by name
#[derive(Args, Debug)]
pub struct FindCommand {
    /// Process name or pattern to search for
    #[arg(required_unless_present = "all")]
    pub name: Option<String>,

    /// List all running processes
    #[arg(long, short = 'a')]
    pub all: bool,

    /// Output as JSON
    #[arg(long, short = 'j')]
    pub json: bool,

    /// Show verbose output with command line and parent PID
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Limit the number of results
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,

    /// Sort by: cpu, mem, pid, name
    #[arg(long, short = 's', default_value = "cpu")]
    pub sort: String,
}

impl FindCommand {
    pub fn execute(&self) -> Result<()> {
        let format = if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };
        let printer = Printer::new(format, self.verbose);

        let mut processes = if self.all {
            Process::find_all()?
        } else if let Some(ref name) = self.name {
            Process::find_by_name(name)?
        } else {
            // This shouldn't happen due to clap validation, but handle it anyway
            Process::find_all()?
        };

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

        printer.print_processes(&processes);
        Ok(())
    }
}
