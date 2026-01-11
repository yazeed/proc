//! Error types for proc CLI
//!
//! Provides structured error handling with helpful suggestions for users.

use thiserror::Error;

/// Main error type for proc operations
#[derive(Error, Debug)]
pub enum ProcError {
    #[error("No process found matching '{0}'\n  Try: proc find --all")]
    ProcessNotFound(String),

    #[error("No process listening on port {0}\n  Try: proc ports")]
    PortNotFound(u16),

    #[error("Permission denied for PID {0}\n  Try: sudo proc <command>")]
    PermissionDenied(u32),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("System error: {0}")]
    SystemError(String),

    #[error("Operation timed out: {0}")]
    Timeout(String),

    #[error("Failed to parse: {0}")]
    ParseError(String),

    #[error("Not supported on this platform: {0}")]
    NotSupported(String),

    #[error("Process {0} is no longer running")]
    ProcessGone(u32),

    #[error("Signal failed: {0}")]
    SignalError(String),
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

/// Result type alias for proc operations
pub type Result<T> = std::result::Result<T, ProcError>;

/// Exit codes for CLI
#[derive(Debug, Clone, Copy)]
pub enum ExitCode {
    Success = 0,
    GeneralError = 1,
    NotFound = 2,
    PermissionDenied = 3,
    InvalidInput = 4,
}

impl From<&ProcError> for ExitCode {
    fn from(err: &ProcError) -> Self {
        match err {
            ProcError::ProcessNotFound(_) | ProcError::PortNotFound(_) => ExitCode::NotFound,
            ProcError::PermissionDenied(_) => ExitCode::PermissionDenied,
            ProcError::InvalidInput(_) => ExitCode::InvalidInput,
            _ => ExitCode::GeneralError,
        }
    }
}
