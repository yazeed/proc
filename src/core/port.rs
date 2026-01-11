//! Port discovery and management
//!
//! Provides cross-platform utilities for discovering which processes
//! are listening on network ports.

use crate::core::Process;
use crate::error::{ProcError, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;

/// Network protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Tcp,
    Udp,
}

/// Information about a listening port
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    /// Port number
    pub port: u16,
    /// Protocol (TCP/UDP)
    pub protocol: Protocol,
    /// Process ID using this port
    pub pid: u32,
    /// Process name
    pub process_name: String,
    /// Bind address (e.g., "0.0.0.0", "127.0.0.1", "::")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

impl PortInfo {
    /// Get all listening ports on the system
    pub fn get_all_listening() -> Result<Vec<PortInfo>> {
        #[cfg(target_os = "macos")]
        {
            Self::get_listening_macos()
        }
        #[cfg(target_os = "linux")]
        {
            Self::get_listening_linux()
        }
        #[cfg(target_os = "windows")]
        {
            Self::get_listening_windows()
        }
    }

    /// Find which process is listening on a specific port
    pub fn find_by_port(port: u16) -> Result<Option<PortInfo>> {
        let ports = Self::get_all_listening()?;
        Ok(ports.into_iter().find(|p| p.port == port))
    }

    /// Get the full process info for this port's process
    pub fn get_process(&self) -> Result<Option<Process>> {
        Process::find_by_pid(self.pid)
    }

    #[cfg(target_os = "macos")]
    fn get_listening_macos() -> Result<Vec<PortInfo>> {
        // Use lsof on macOS - only TCP LISTEN sockets
        let output = Command::new("lsof")
            .args(["-iTCP", "-sTCP:LISTEN", "-P", "-n"])
            .output()
            .map_err(|e| ProcError::SystemError(format!("Failed to run lsof: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut ports = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for line in stdout.lines().skip(1) {
            // Skip header
            if let Some(port_info) = Self::parse_lsof_line(line) {
                // Deduplicate (same port can appear multiple times for IPv4/IPv6)
                let key = (port_info.port, port_info.pid);
                if seen.insert(key) {
                    ports.push(port_info);
                }
            }
        }

        Ok(ports)
    }

    #[cfg(target_os = "macos")]
    fn parse_lsof_line(line: &str) -> Option<PortInfo> {
        // lsof output format:
        // COMMAND  PID USER  FD  TYPE  DEVICE  SIZE/OFF  NODE  NAME
        // rapportd 643 zee   8u  IPv4  0x...   0t0       TCP   *:52633 (LISTEN)
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 9 {
            return None;
        }

        let process_name = parts[0].to_string();
        let pid: u32 = parts[1].parse().ok()?;

        // Find the NAME column - it's after the NODE (TCP/UDP) column
        // The NAME looks like "*:3000" or "127.0.0.1:8080" or "*:52633 (LISTEN)"
        // Find the column that contains a colon and looks like an address:port
        let name_col = parts.iter().skip(8).find(|p| p.contains(':'))?;

        // Remove any trailing state like "(LISTEN)" by taking just the address:port part
        let addr_port =
            name_col.trim_end_matches(|c: char| c == ')' || c.is_alphabetic() || c == '(');

        // Split address and port
        let last_colon = addr_port.rfind(':')?;
        let port_str = &addr_port[last_colon + 1..];
        let port: u16 = port_str.parse().ok()?;

        let addr_part = &addr_port[..last_colon];
        let address = Some(if addr_part == "*" || addr_part.is_empty() {
            "0.0.0.0".to_string()
        } else {
            addr_part.to_string()
        });

        Some(PortInfo {
            port,
            protocol: Protocol::Tcp,
            pid,
            process_name,
            address,
        })
    }

    #[cfg(target_os = "linux")]
    fn get_listening_linux() -> Result<Vec<PortInfo>> {
        // Use ss on Linux (more modern than netstat)
        let output = Command::new("ss")
            .args(["-tlnp"])
            .output()
            .map_err(|e| ProcError::SystemError(format!("Failed to run ss: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut ports = Vec::new();

        for line in stdout.lines().skip(1) {
            if let Some(port_info) = Self::parse_ss_line(line) {
                ports.push(port_info);
            }
        }

        Ok(ports)
    }

    #[cfg(target_os = "linux")]
    fn parse_ss_line(line: &str) -> Option<PortInfo> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 6 {
            return None;
        }

        // Local address is typically in column 4 (e.g., "0.0.0.0:22" or "*:80")
        let local_addr = parts[3];
        let port_str = local_addr.rsplit(':').next()?;
        let port: u16 = port_str.parse().ok()?;

        let address = local_addr.rsplit(':').nth(1).map(|s| {
            if s == "*" {
                "0.0.0.0".to_string()
            } else {
                s.to_string()
            }
        });

        // Process info is in the last column, format: users:(("name",pid=1234,fd=5))
        let proc_info = parts.last()?;
        let pid = Self::extract_pid_from_ss(proc_info)?;
        let process_name =
            Self::extract_name_from_ss(proc_info).unwrap_or_else(|| "unknown".to_string());

        Some(PortInfo {
            port,
            protocol: Protocol::Tcp,
            pid,
            process_name,
            address,
        })
    }

    #[cfg(target_os = "linux")]
    fn extract_pid_from_ss(info: &str) -> Option<u32> {
        // Format: users:(("sshd",pid=1234,fd=3))
        let pid_marker = "pid=";
        let start = info.find(pid_marker)? + pid_marker.len();
        let rest = &info[start..];
        let end = rest.find(|c: char| !c.is_ascii_digit())?;
        rest[..end].parse().ok()
    }

    #[cfg(target_os = "linux")]
    fn extract_name_from_ss(info: &str) -> Option<String> {
        // Format: users:(("sshd",pid=1234,fd=3))
        let start = info.find("((\"")? + 3;
        let rest = &info[start..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }

    #[cfg(target_os = "windows")]
    fn get_listening_windows() -> Result<Vec<PortInfo>> {
        // Use netstat on Windows
        let output = Command::new("netstat")
            .args(["-ano", "-p", "TCP"])
            .output()
            .map_err(|e| ProcError::SystemError(format!("Failed to run netstat: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut ports = Vec::new();

        for line in stdout.lines() {
            if line.contains("LISTENING") {
                if let Some(port_info) = Self::parse_netstat_line(line) {
                    ports.push(port_info);
                }
            }
        }

        Ok(ports)
    }

    #[cfg(target_os = "windows")]
    fn parse_netstat_line(line: &str) -> Option<PortInfo> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            return None;
        }

        // Local address is column 2 (e.g., "0.0.0.0:135")
        let local_addr = parts[1];
        let port_str = local_addr.rsplit(':').next()?;
        let port: u16 = port_str.parse().ok()?;

        let address = local_addr.rsplit(':').nth(1).map(String::from);

        // PID is the last column
        let pid: u32 = parts.last()?.parse().ok()?;

        // Get process name from PID
        let process_name =
            Self::get_process_name_windows(pid).unwrap_or_else(|| "unknown".to_string());

        Some(PortInfo {
            port,
            protocol: Protocol::Tcp,
            pid,
            process_name,
            address,
        })
    }

    #[cfg(target_os = "windows")]
    fn get_process_name_windows(pid: u32) -> Option<String> {
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout.lines().next()?;
        let name = line.split(',').next()?;
        Some(name.trim_matches('"').to_string())
    }
}

/// Parse a port from various formats (":3000", "3000", etc.)
pub fn parse_port(input: &str) -> Result<u16> {
    let cleaned = input.trim().trim_start_matches(':');
    cleaned
        .parse()
        .map_err(|_| ProcError::InvalidInput(format!("Invalid port: '{}'", input)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_port() {
        assert_eq!(parse_port(":3000").unwrap(), 3000);
        assert_eq!(parse_port("3000").unwrap(), 3000);
        assert_eq!(parse_port("  :8080  ").unwrap(), 8080);
    }

    #[test]
    fn test_parse_port_invalid() {
        assert!(parse_port("abc").is_err());
        assert!(parse_port("").is_err());
    }

    #[test]
    fn test_get_listening_ports() {
        // This test may or may not find ports depending on the system
        let result = PortInfo::get_all_listening();
        assert!(result.is_ok());
    }
}
