//! Error types for proc CLI
//!
//! Provides structured error handling with helpful suggestions for users.

use thiserror::Error;

/// Main error type for proc operations
#[derive(Error, Debug)]
pub enum ProcError {
    /// No process found matching the given target
    #[error("No process found matching '{0}'\n  Try: proc list to list all processes")]
    ProcessNotFound(String),

    /// No process is listening on the specified port
    #[error("No process listening on port {0}\n  Try: proc ports")]
    PortNotFound(u16),

    /// Insufficient permissions to operate on the process
    #[error("Permission denied for PID {0}\n  Try: sudo proc <command>")]
    PermissionDenied(u32),

    /// User provided invalid input or arguments
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// An underlying system call failed
    #[error("System error: {0}")]
    SystemError(String),

    /// The operation exceeded the allowed time limit
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Failed to parse input or system output
    #[error("Failed to parse: {0}")]
    ParseError(String),

    /// Feature is not available on the current platform
    #[error("Not supported on this platform: {0}")]
    NotSupported(String),

    /// The target process terminated during the operation
    #[error("Process {0} is no longer running")]
    ProcessGone(u32),

    /// Failed to send a signal to the process
    #[error("Signal failed: {0}")]
    SignalError(String),

    /// The command already printed its result, but the operation did not fully
    /// succeed (some processes failed, a port stayed busy, a wait timed out).
    /// `main` exits with `exit_code` without printing anything further.
    #[error("{message}")]
    Reported {
        /// Human-readable summary of what did not succeed
        message: String,
        /// Exit code to return
        exit_code: ExitCode,
    },
}

impl ProcError {
    /// Creates a [`ProcError::Reported`] for a result that has already been printed.
    pub fn reported(exit_code: ExitCode, message: impl Into<String>) -> Self {
        ProcError::Reported {
            message: message.into(),
            exit_code,
        }
    }
}

impl From<std::io::Error> for ProcError {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::PermissionDenied => ProcError::PermissionDenied(0),
            std::io::ErrorKind::NotFound => ProcError::ProcessNotFound("unknown".to_string()),
            _ => ProcError::SystemError(err.to_string()),
        }
    }
}

impl From<serde_json::Error> for ProcError {
    fn from(err: serde_json::Error) -> Self {
        ProcError::ParseError(err.to_string())
    }
}

impl From<regex::Error> for ProcError {
    fn from(err: regex::Error) -> Self {
        ProcError::InvalidInput(format!("Invalid pattern: {}", err))
    }
}

impl From<dialoguer::Error> for ProcError {
    fn from(err: dialoguer::Error) -> Self {
        ProcError::SystemError(format!("Dialog error: {}", err))
    }
}

/// Result type alias for proc operations
pub type Result<T> = std::result::Result<T, ProcError>;

/// Exit codes for CLI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    /// Operation completed successfully
    Success = 0,
    /// A general error occurred
    GeneralError = 1,
    /// The requested process or port was not found
    NotFound = 2,
    /// Operation requires elevated privileges
    PermissionDenied = 3,
    /// Invalid arguments or input provided
    InvalidInput = 4,
}

impl From<&ProcError> for ExitCode {
    fn from(err: &ProcError) -> Self {
        match err {
            ProcError::ProcessNotFound(_) | ProcError::PortNotFound(_) => ExitCode::NotFound,
            ProcError::PermissionDenied(_) => ExitCode::PermissionDenied,
            ProcError::InvalidInput(_) => ExitCode::InvalidInput,
            ProcError::Reported { exit_code, .. } => *exit_code,
            _ => ExitCode::GeneralError,
        }
    }
}
