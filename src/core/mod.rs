//! Core abstractions for process and port management
//!
//! This module provides cross-platform abstractions for working with
//! system processes and network ports.

pub mod filters;
pub mod port;
pub mod process;
pub mod target;

pub use filters::{
    apply_filters, matches_by_filter, matches_in_filter, resolve_in_dir, sort_processes,
    PortSortKey, SortKey,
};
pub use port::{parse_port, PortInfo, Protocol};
#[cfg(unix)]
pub use process::parse_signal_name;
pub use process::{Process, ProcessStatus};
pub use target::{
    find_ports_for_pid, parse_target, parse_targets, resolve_target, resolve_target_single,
    resolve_targets, resolve_targets_excluding_self, TargetType,
};
