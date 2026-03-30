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
//! - **Working Directory**: See which project folder a process is running from
//! - **Terminal-Adaptive Tables**: Tables adjust to terminal width automatically
//! - **Consistent Filters**: `--in`, `--by`, `--min-uptime`, `--parent`, `--range`, `--sort`, `--limit`
//! - **JSON Output**: `--json` on every command with consistent `{action, success, ...}` envelopes
//! - **Custom Signals**: `proc stop nginx --signal HUP` - send any signal via `--signal`
//! - **Real-Time Monitoring**: `proc watch` / `proc top` - live process table (interactive, TTY only)
//! - **Process Waiting**: `proc wait node` - block until process(es) exit (pipe-friendly)
//! - **Freeze/Thaw**: `proc freeze` / `proc thaw` - pause and resume by port, PID, or name
//! - **Port Freeing**: `proc free :3000` - kill and verify port is available
//! - **Ancestry Tracing**: `proc why :3000` - trace why a port is busy
//! - **Orphan Detection**: `proc orphans` - find abandoned child processes
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
//! # Watch processes in real-time
//! proc watch node --in .
//!
//! # Wait for a process to finish
//! proc wait node --timeout 3600
//!
//! # Free a port (kill + verify available)
//! proc free :3000
//!
//! # Why is this port busy?
//! proc why :3000
//!
//! # Pause and resume
//! proc freeze node && proc thaw node
//!
//! # Send custom signal
//! proc stop nginx --signal HUP
//!
//! # JSON output for scripting/LLMs
//! proc by node --json
//!
//! # Generate shell completions
//! proc completions zsh > ~/.zsh/completions/_proc
//! ```
//!
//! ## Commands
//!
//! **Discovery**: `on`, `for`, `by`, `in`, `list`, `info`, `ports`, `tree`, `stuck`, `why`, `orphans`
//!
//! **Lifecycle**: `kill`, `stop`, `unstick`, `freeze`, `thaw`, `free`
//!
//! **Monitoring**: `watch` (interactive TUI), `wait` (blocking, pipe-friendly)
//!
//! **Tooling**: `completions`, `manpage`

pub mod commands;
pub mod core;
pub mod error;
pub mod ui;

pub use error::{ProcError, Result};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
