//! Shared filter utilities used across commands
//!
//! Provides common filter resolution logic to avoid duplication.

use crate::core::{Process, ProcessStatus};
use clap::ValueEnum;
use std::path::PathBuf;

/// Values accepted by `--status`. An unknown value is a usage error (exit 4),
/// not a filter that silently matches everything.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum StatusFilter {
    /// Running on a CPU
    Running,
    /// Waiting (idle, blocked on I/O)
    #[value(alias = "sleep")]
    Sleeping,
    /// Paused (e.g. by `proc freeze` or Ctrl+Z)
    #[value(alias = "stop")]
    Stopped,
    /// Exited, but not yet reaped by its parent
    Zombie,
}

impl StatusFilter {
    /// Whether a process with this status passes the filter
    pub fn matches(self, status: ProcessStatus) -> bool {
        matches!(
            (self, status),
            (StatusFilter::Running, ProcessStatus::Running)
                | (StatusFilter::Sleeping, ProcessStatus::Sleeping)
                | (StatusFilter::Stopped, ProcessStatus::Stopped)
                | (StatusFilter::Zombie, ProcessStatus::Zombie)
        )
    }
}

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

/// Turn a Windows verbatim path into the plain form processes report.
///
/// `canonicalize` on Windows returns `\\?\D:\app\x.exe` (or
/// `\\?\UNC\server\share\x.exe`), while a process's executable path is
/// `D:\app\x.exe` (`\\server\share\x.exe`), so the two never compare equal.
/// Any other form comes back unchanged. Pure string work, so it is tested on
/// every OS; only Windows calls it.
pub fn strip_verbatim_prefix(path: &str) -> String {
    if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{unc}");
    }
    if let Some(disk) = path.strip_prefix(r"\\?\") {
        let mut chars = disk.chars();
        let is_disk = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            && chars.next() == Some(':')
            && matches!(chars.next(), Some('\\') | None);
        if is_disk {
            return disk.to_string();
        }
    }
    path.to_string()
}

#[cfg(test)]
mod verbatim_tests {
    use super::strip_verbatim_prefix;

    #[test]
    fn a_verbatim_disk_path_loses_its_prefix() {
        assert_eq!(
            strip_verbatim_prefix(r"\\?\D:\a\proc-cli\listener.exe"),
            r"D:\a\proc-cli\listener.exe"
        );
    }

    #[test]
    fn a_verbatim_unc_path_becomes_a_plain_unc_path() {
        assert_eq!(
            strip_verbatim_prefix(r"\\?\UNC\server\share\listener.exe"),
            r"\\server\share\listener.exe"
        );
    }

    #[test]
    fn other_forms_are_unchanged() {
        for path in [
            r"D:\a\proc-cli\listener.exe",
            r"\\server\share\listener.exe",
            r"\\?\Volume{0123}\listener.exe",
            "/usr/bin/node",
            "",
        ] {
            assert_eq!(strip_verbatim_prefix(path), path);
        }
    }
}
