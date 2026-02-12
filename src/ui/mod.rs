//! User interface components for proc CLI
//!
//! Handles output formatting, colors, and interactive prompts.

pub mod format;
pub mod output;

pub use format::{colorize_status, format_duration, format_memory, truncate_path, truncate_string};
pub use output::{OutputFormat, Printer};
