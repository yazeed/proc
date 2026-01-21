#![warn(missing_docs)]
//! # proc - Semantic Process Management CLI
//!
//! `proc` is a semantic command-line tool that makes process management
//! intuitive, cross-platform, and AI-centric.
//!
//! ## Features
//!
//! - **Semantic Commands**: Commands mean what they say (`proc kill node`)
//! - **Cross-Platform**: Works on macOS, Linux, and Windows
//! - **Process Lifecycle**: DISCOVER → INSPECT → MANAGE → MONITOR → REMEDIATE
//! - **Beautiful Output**: Colored terminal output and JSON for scripting
//!
//! ## Example
//!
//! ```bash
//! # List processes
//! proc list node
//!
//! # What's on a port?
//! proc on :3000
//!
//! # List all listening ports
//! proc ports
//!
//! # Kill a process
//! proc kill node
//!
//! # Find stuck processes
//! proc stuck
//! ```

pub mod commands;
pub mod core;
pub mod error;
pub mod ui;

pub use error::{ProcError, Result};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
