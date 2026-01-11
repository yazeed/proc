//! Core abstractions for process and port management
//!
//! This module provides cross-platform abstractions for working with
//! system processes and network ports.

pub mod port;
pub mod process;

pub use port::{PortInfo, Protocol};
pub use process::{Process, ProcessStatus};
