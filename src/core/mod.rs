//! Core abstractions for process and port management
//!
//! This module provides cross-platform abstractions for working with
//! system processes and network ports.

pub mod apply;
pub mod filters;
pub mod name_match;
pub mod port;
pub mod process;
pub mod target;

pub use apply::apply_to_each;
pub use filters::{
    apply_filters, matches_by_filter, matches_in_filter, resolve_in_dir, sort_processes,
    PortSortKey, SortKey, StatusFilter,
};
pub use name_match::{classify, matches_name, Hit, NameMatch, Names};
pub use port::{parse_port, PortInfo, Protocol};
#[cfg(unix)]
pub use process::parse_signal_name;
pub use process::{escape_control, is_thread, Process, ProcessStatus};
pub use target::{
    find_ports_for_pid, flatten_targets, parse_target, parse_targets, resolve_target,
    resolve_targets, resolve_targets_excluding_self, validate_target_arg, Resolved, TargetType,
};
