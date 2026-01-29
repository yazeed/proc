//! CLI command implementations
//!
//! Each command is implemented in its own module and follows a consistent pattern:
//! - Parse arguments from clap
//! - Execute the operation
//! - Format and display results

pub mod by;
pub mod find_in;
pub mod info;
pub mod kill;
pub mod list;
pub mod on;
pub mod ports;
pub mod stop;
pub mod stuck;
pub mod tree;
pub mod unstick;

pub use by::ByCommand;
pub use find_in::InCommand;
pub use info::InfoCommand;
pub use kill::KillCommand;
pub use list::ListCommand;
pub use on::OnCommand;
pub use ports::PortsCommand;
pub use stop::StopCommand;
pub use stuck::StuckCommand;
pub use tree::TreeCommand;
pub use unstick::UnstickCommand;
