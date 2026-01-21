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
