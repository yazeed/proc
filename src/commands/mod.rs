//! CLI command implementations
//!
//! Each command is implemented in its own module and follows a consistent pattern:
//! - Parse arguments from clap
//! - Execute the operation
//! - Format and display results

pub mod find;
pub mod kill;
pub mod on;
pub mod ports;
pub mod stuck;

pub use find::FindCommand;
pub use kill::KillCommand;
pub use on::OnCommand;
pub use ports::PortsCommand;
pub use stuck::StuckCommand;
