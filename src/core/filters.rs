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
        SortKey::Name => processes.sort_by_key(|p| p.name.to_lowercase()),
    }
}

/// Check if a process matches a `--by` name filter.
///
/// Matches against both the process name AND full command line (case-insensitive),
/// consistent with how positional targets work via `find_by_name`.
pub fn matches_by_filter(process: &Process, pattern: &str) -> bool {
    let pattern_lower = pattern.to_lowercase();
    if process.name.to_lowercase().contains(&pattern_lower) {
        return true;
    }
    if let Some(ref cmd) = process.command {
        if cmd.to_lowercase().contains(&pattern_lower) {
            return true;
        }
    }
    false
}

/// Check if a process matches an `--in` directory filter.
pub fn matches_in_filter(process: &Process, dir_path: &std::path::Path) -> bool {
    if let Some(ref cwd) = process.cwd {
        PathBuf::from(cwd).starts_with(dir_path)
    } else {
        false
    }
}

/// Apply `--in` and `--by` filters to a list of processes (in a single pass).
///
/// This replaces the 14-line retain block duplicated across commands.
pub fn apply_filters(
    processes: &mut Vec<Process>,
    in_dir: &Option<String>,
    by_name: &Option<String>,
) {
    let in_dir_filter = resolve_in_dir(in_dir);
    processes.retain(|p| {
        if let Some(ref dir_path) = in_dir_filter {
            if !matches_in_filter(p, dir_path) {
                return false;
            }
        }
        if let Some(ref name) = by_name {
            if !matches_by_filter(p, name) {
                return false;
            }
        }
        true
    });
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
