//! `proc ports` - List all listening ports
//!
//! Examples:
//!   proc ports              # Show all listening ports
//!   proc ports --filter node # Filter by process name

use crate::core::PortInfo;
use crate::error::Result;
use crate::ui::{OutputFormat, Printer};
use clap::Args;

/// List all listening ports
#[derive(Args, Debug)]
pub struct PortsCommand {
    /// Filter by process name
    #[arg(long, short = 'f')]
    pub filter: Option<String>,

    /// Output as JSON
    #[arg(long, short = 'j')]
    pub json: bool,

    /// Show verbose output
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Sort by: port, pid, name
    #[arg(long, short = 's', default_value = "port")]
    pub sort: String,
}

impl PortsCommand {
    pub fn execute(&self) -> Result<()> {
        let format = if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };
        let printer = Printer::new(format, self.verbose);

        let mut ports = PortInfo::get_all_listening()?;

        // Filter by process name if specified
        if let Some(ref filter) = self.filter {
            let filter_lower = filter.to_lowercase();
            ports.retain(|p| p.process_name.to_lowercase().contains(&filter_lower));
        }

        // Sort ports
        match self.sort.to_lowercase().as_str() {
            "port" => ports.sort_by_key(|p| p.port),
            "pid" => ports.sort_by_key(|p| p.pid),
            "name" => ports.sort_by(|a, b| {
                a.process_name
                    .to_lowercase()
                    .cmp(&b.process_name.to_lowercase())
            }),
            _ => ports.sort_by_key(|p| p.port),
        }

        printer.print_ports(&ports);
        Ok(())
    }
}
