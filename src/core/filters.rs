//! Shared filter utilities used across commands
//!
//! Provides common filter resolution logic to avoid duplication.

use std::path::PathBuf;

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
