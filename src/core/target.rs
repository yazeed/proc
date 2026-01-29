//! Target resolution - Convert user input to processes
//!
//! Targets can be:
//! - `:port` - Process listening on this port
//! - `pid` - Process with this PID (numeric)
//! - `name` - Processes matching this name

use crate::core::port::{parse_port, PortInfo};
use crate::core::Process;
use crate::error::{ProcError, Result};

/// Resolved target type
#[derive(Debug, Clone)]
pub enum TargetType {
    /// Target a process by the port it listens on (e.g., `:3000`)
    Port(u16),
    /// Target a process by its process ID (e.g., `1234`)
    Pid(u32),
    /// Target processes by name pattern (e.g., `node`)
    Name(String),
}

/// Parse a target string and determine its type
pub fn parse_target(target: &str) -> TargetType {
    let target = target.trim();

    // Explicit port prefix
    if target.starts_with(':') {
        if let Ok(port) = parse_port(target) {
            return TargetType::Port(port);
        }
    }

    // Pure number - treat as PID
    if let Ok(pid) = target.parse::<u32>() {
        return TargetType::Pid(pid);
    }

    // Otherwise it's a name
    TargetType::Name(target.to_string())
}

/// Resolve a target to processes
pub fn resolve_target(target: &str) -> Result<Vec<Process>> {
    match parse_target(target) {
        TargetType::Port(port) => resolve_port(port),
        TargetType::Pid(pid) => resolve_pid(pid),
        TargetType::Name(name) => Process::find_by_name(&name),
    }
}

/// Resolve a single target to exactly one process
pub fn resolve_target_single(target: &str) -> Result<Process> {
    let processes = resolve_target(target)?;

    if processes.is_empty() {
        return Err(ProcError::ProcessNotFound(target.to_string()));
    }

    if processes.len() > 1 {
        return Err(ProcError::InvalidInput(format!(
            "Target '{}' matches {} processes. Be more specific.",
            target,
            processes.len()
        )));
    }

    Ok(processes.into_iter().next().unwrap())
}

/// Resolve port to process
fn resolve_port(port: u16) -> Result<Vec<Process>> {
    match PortInfo::find_by_port(port)? {
        Some(port_info) => match Process::find_by_pid(port_info.pid)? {
            Some(proc) => Ok(vec![proc]),
            None => Err(ProcError::ProcessGone(port_info.pid)),
        },
        None => Err(ProcError::PortNotFound(port)),
    }
}

/// Resolve PID to process
fn resolve_pid(pid: u32) -> Result<Vec<Process>> {
    match Process::find_by_pid(pid)? {
        Some(proc) => Ok(vec![proc]),
        None => Err(ProcError::ProcessNotFound(pid.to_string())),
    }
}

/// Find all ports a process is listening on
pub fn find_ports_for_pid(pid: u32) -> Result<Vec<PortInfo>> {
    let all_ports = PortInfo::get_all_listening()?;
    Ok(all_ports.into_iter().filter(|p| p.pid == pid).collect())
}

/// Split comma-separated targets into individual target strings
///
/// Examples:
///   ":3000,:8080" -> [":3000", ":8080"]
///   "node,python" -> ["node", "python"]
///   ":3000, 1234, node" -> [":3000", "1234", "node"]
pub fn parse_targets(targets_str: &str) -> Vec<String> {
    targets_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Resolve multiple targets, deduplicating by PID
///
/// Returns a tuple of (found processes, not found target strings)
pub fn resolve_targets(targets: &[String]) -> (Vec<Process>, Vec<String>) {
    use std::collections::HashSet;

    let mut all_processes = Vec::new();
    let mut seen_pids = HashSet::new();
    let mut not_found = Vec::new();

    for target in targets {
        match resolve_target(target) {
            Ok(processes) => {
                for proc in processes {
                    if seen_pids.insert(proc.pid) {
                        all_processes.push(proc);
                    }
                }
            }
            Err(_) => not_found.push(target.clone()),
        }
    }

    (all_processes, not_found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_targets_single() {
        assert_eq!(parse_targets(":3000"), vec![":3000"]);
        assert_eq!(parse_targets("node"), vec!["node"]);
        assert_eq!(parse_targets("1234"), vec!["1234"]);
    }

    #[test]
    fn test_parse_targets_multiple() {
        assert_eq!(parse_targets(":3000,:8080"), vec![":3000", ":8080"]);
        assert_eq!(parse_targets("node,python"), vec!["node", "python"]);
        assert_eq!(
            parse_targets(":3000,1234,node"),
            vec![":3000", "1234", "node"]
        );
    }

    #[test]
    fn test_parse_targets_with_whitespace() {
        assert_eq!(
            parse_targets(":3000, :8080, :9000"),
            vec![":3000", ":8080", ":9000"]
        );
        assert_eq!(parse_targets(" node , python "), vec!["node", "python"]);
    }

    #[test]
    fn test_parse_targets_empty_entries() {
        assert_eq!(parse_targets(":3000,,,:8080"), vec![":3000", ":8080"]);
        assert_eq!(parse_targets(",,node,,"), vec!["node"]);
    }

    #[test]
    fn test_parse_target_port() {
        assert!(matches!(parse_target(":3000"), TargetType::Port(3000)));
        assert!(matches!(parse_target(":8080"), TargetType::Port(8080)));
    }

    #[test]
    fn test_parse_target_pid() {
        assert!(matches!(parse_target("1234"), TargetType::Pid(1234)));
        assert!(matches!(parse_target("99999"), TargetType::Pid(99999)));
    }

    #[test]
    fn test_parse_target_name() {
        assert!(matches!(parse_target("node"), TargetType::Name(_)));
        assert!(matches!(parse_target("my-process"), TargetType::Name(_)));
    }
}
