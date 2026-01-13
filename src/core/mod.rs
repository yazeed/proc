//! Core abstractions for process and port management
//!
//! This module provides cross-platform abstractions for working with
//! system processes and network ports.

pub mod port;
pub mod process;
pub mod target;

pub use port::{parse_port, PortInfo, Protocol};
pub use process::{Process, ProcessStatus};
pub use target::{
    find_ports_for_pid, parse_target, resolve_target, resolve_target_single, TargetType,
};
