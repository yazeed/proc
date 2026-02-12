//! Shared filter utilities used across commands
//!
//! Provides common filter resolution logic to avoid duplication.

use crate::core::Process;
use clap::ValueEnum;
use std::path::PathBuf;

/// Sort key for process commands (list, by, in, for)
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SortKey {
    /// Sort by CPU usage (descending)
    Cpu,
    /// Sort by memory usage (descending)
    Mem,
    /// Sort by process ID (ascending)
    Pid,
    /// Sort by process name (ascending)
    Name,
}

/// Sort key for port commands
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PortSortKey {
    /// Sort by port number (ascending)
    Port,
    /// Sort by process ID (ascending)
    Pid,
    /// Sort by process name (ascending)
    Name,
}

/// Sort a list of processes by the given key.
pub fn sort_processes(processes: &mut [Process], key: SortKey) {
    match key {
        SortKey::Cpu => processes.sort_by(|a, b| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        SortKey::Mem => processes.sort_by(|a, b| {
            b.memory_mb
                .partial_cmp(&a.memory_mb)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        SortKey::Pid => processes.sort_by_key(|p| p.pid),
        SortKey::Name => {
            processes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        }
    }
}

/// Resolve an `--in` directory filter to an absolute PathBuf.
///
/// Handles "." (current directory), relative paths, and absolute paths.
pub fn resolve_in_dir(in_dir: &Option<String>) -> Option<PathBuf> {
    in_dir.as_ref().map(|p| {
        if p == "." {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        } else {
            let path = PathBuf::from(p);
            if path.is_relative() {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(path)
            } else {
                path
            }
        }
    })
}
