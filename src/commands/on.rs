//! `proc on` - Find what process is on a specific port
//!
//! Examples:
//!   proc on :3000           # What's on port 3000?
//!   proc on 8080            # Port number alone works too

use crate::core::port::{parse_port, PortInfo};
use crate::error::{ProcError, Result};
use crate::ui::{OutputFormat, Printer};
use clap::Args;

/// Show what process is on a port
#[derive(Args, Debug)]
pub struct OnCommand {
    /// Port number (e.g., :3000 or 3000)
    pub port: String,

    /// Output as JSON
    #[arg(long, short = 'j')]
    pub json: bool,

    /// Show verbose output
    #[arg(long, short = 'v')]
    pub verbose: bool,
}

impl OnCommand {
    pub fn execute(&self) -> Result<()> {
        let format = if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };
        let printer = Printer::new(format, self.verbose);

        let port = parse_port(&self.port)?;

        match PortInfo::find_by_port(port)? {
            Some(port_info) => {
                printer.print_port_info(&port_info);
                Ok(())
            }
            None => Err(ProcError::PortNotFound(port)),
        }
    }
}
