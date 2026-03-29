//! Shared formatting utilities used across commands and output
//!
//! Provides common string formatting, truncation, and colorization.

use crate::core::ProcessStatus;
use colored::*;

/// Format a duration in seconds to a human-readable string.
///
/// Examples: "45s", "3m 12s", "2h 15m", "1d 6h"
pub fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86400, (secs % 86400) / 3600)
    }
}

/// Return "es" for plural process counts, "" for singular.
///
/// Usage: `format!("{} process{}", n, plural(n))` → "1 process" or "3 processes"
pub fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "es"
    }
}

/// Truncate a string to a maximum length, appending "..." if truncated.
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Truncate a path intelligently — shows the end (most relevant part).
pub fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_string()
    } else {
        let start = path.len().saturating_sub(max_len.saturating_sub(3));
        format!("...{}", &path[start..])
    }
}

/// Format memory in MB to a human-readable string with adaptive units.
///
/// Examples: "512KB", "6.0MB", "3.0GB"
pub fn format_memory(memory_mb: f64) -> String {
    if memory_mb < 1.0 {
        format!("{:.0}KB", memory_mb * 1000.0)
    } else if memory_mb < 1000.0 {
        format!("{:.1}MB", memory_mb)
    } else {
        format!("{:.1}GB", memory_mb / 1000.0)
    }
}

/// Colorize a process status string based on the status variant.
pub fn colorize_status(status: &ProcessStatus, status_str: &str) -> ColoredString {
    match status {
        ProcessStatus::Running => status_str.green(),
        ProcessStatus::Sleeping => status_str.blue(),
        ProcessStatus::Stopped => status_str.yellow(),
        ProcessStatus::Zombie => status_str.red(),
        _ => status_str.white(),
    }
}
