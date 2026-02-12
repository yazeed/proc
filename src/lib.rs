#![warn(missing_docs)]
//! # proc - Semantic Process Management CLI
//!
//! Semantic CLI tool for process management. Target by port, PID, name or path.
//!
//! ## Features
//!
//! - **Unified Targets**: `:port`, `PID`, and `name` work the same everywhere
//! - **Multi-Target**: `proc kill :3000,:8080,node` - comma-separated targets
//! - **Query Language**: `proc by node --in .` - composable filters
//! - **Terminal-Adaptive Tables**: Tables adjust to terminal width automatically
//! - **Consistent Filters**: `--in`, `--by`, `--min-uptime`, `--parent`, `--range`, `--sort`, `--limit`
//! - **File Lookup**: `proc for ./script.py` - find by file path
//! - **Cross-Platform**: macOS, Linux, and Windows
//! - **Shell Completions**: bash, zsh, fish via `proc completions`
//! - **Man Pages**: `proc manpage` generates documentation
//!
//! ## Quick Start
//!
//! ```bash
//! # What's on port 3000?
//! proc on :3000
//!
//! # What's running this file?
//! proc for ./script.py
//!
//! # Kill multiple targets
//! proc kill :3000,:8080,node
//!
//! # Node processes in current directory
//! proc by node --in .
//!
//! # Preview before killing
//! proc kill node --dry-run
//!
//! # Kill only node processes in current directory
//! proc kill node --in .
//!
//! # Generate shell completions
//! proc completions zsh > ~/.zsh/completions/_proc
//! ```
//!
//! ## Commands
//!
//! **Discovery**: `on`, `for`, `by`, `in`, `list`, `info`, `ports`, `tree`, `stuck`
//!
//! **Lifecycle**: `kill`, `stop`, `unstick`
//!
//! **Tooling**: `completions`, `manpage`

pub mod commands;
pub mod core;
pub mod error;
pub mod ui;

pub use error::{ProcError, Result};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
