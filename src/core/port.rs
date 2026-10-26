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
    /// Transmission Control Protocol - reliable, ordered delivery
    Tcp,
    /// User Datagram Protocol - fast, connectionless delivery
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
        // Use lsof on macOS - only TCP LISTEN sockets. `+c 0` prints the whole
        // command name; by default lsof cuts it to 9 characters
        let output = Command::new("lsof")
            .args(["+c", "0", "-iTCP", "-sTCP:LISTEN", "-P", "-n"])
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

    /// lsof writes characters it can't print in a column, spaces included, as
    /// \xNN, so "Activity Monitor" arrives as "Activity\x20Monitor"
    #[cfg(target_os = "macos")]
    fn unescape_lsof(name: &str) -> String {
        let bytes = name.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\\' && bytes.get(i + 1) == Some(&b'x') && i + 3 < bytes.len() {
                if let Ok(b) = u8::from_str_radix(&name[i + 2..i + 4], 16) {
                    out.push(b);
                    i += 4;
                    continue;
                }
            }
            out.push(bytes[i]);
            i += 1;
        }
        String::from_utf8_lossy(&out).into_owned()
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

        let process_name = Self::unescape_lsof(parts[0]);
        let pid: u32 = parts[1].parse().ok()?;

        // Find the NAME column - it's after the NODE (TCP/UDP) column
        // The NAME looks like "*:3000" or "127.0.0.1:8080" or "*:52633 (LISTEN)"
        // Find the column that contains a colon and looks like an address:port
        let name_col = parts.iter().skip(8).find(|p| p.contains(':'))?;

        // Remove any trailing state like "(LISTEN)" by taking just the address:port part
        let addr_port =
            name_col.trim_end_matches(|c: char| c == ')' || c.is_alphabetic() || c == '(');

        let (address, port) = split_listen_address(addr_port)?;

        Some(PortInfo {
            port,
            protocol: Protocol::Tcp,
            pid,
            process_name,
            address: Some(address),
        })
    }

    #[cfg(target_os = "linux")]
    fn get_listening_linux() -> Result<Vec<PortInfo>> {
        // Use ss on Linux (more modern than netstat)
        let output = Command::new("ss").args(["-tlnp"]).output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ProcError::SystemError(
                    "Port lookups on Linux need `ss`, from the iproute2 package \
                     (e.g. apt install iproute2, apk add iproute2)"
                        .to_string(),
                )
            } else {
                ProcError::SystemError(format!("Failed to run ss: {}", e))
            }
        })?;

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

        // Local address is typically in column 4 (e.g., "0.0.0.0:22", "*:80" or "[::1]:631")
        let (address, port) = split_listen_address(parts[3])?;

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
            address: Some(address),
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

        // Local address is column 2 (e.g., "0.0.0.0:135" or "[::]:135")
        let (address, port) = split_listen_address(parts[1])?;

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
            address: Some(address),
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

/// Split a listening socket's local address, as ss, lsof and netstat print it,
/// into the bind address and the port. IPv6 addresses come in brackets
/// (`[::1]:631`, `[fe80::1%eth0]:22`) and are returned without them; `*`, every
/// address, is returned as `0.0.0.0`.
fn split_listen_address(local: &str) -> Option<(String, u16)> {
    let (host, port) = local.rsplit_once(':')?;
    let port = port.parse().ok()?;
    let host = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    let address = if host.is_empty() || host == "*" {
        "0.0.0.0"
    } else {
        host
    };
    Some((address.to_string(), port))
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
    fn test_split_listen_address() {
        for (local, address, port) in [
            ("0.0.0.0:22", "0.0.0.0", 22),
            ("127.0.0.1:8080", "127.0.0.1", 8080),
            ("*:80", "0.0.0.0", 80),
            ("[::]:80", "::", 80),
            ("[::1]:631", "::1", 631),
            ("[::ffff:127.0.0.1]:8080", "::ffff:127.0.0.1", 8080),
            ("[fe80::1%eth0]:22", "fe80::1%eth0", 22),
            ("127.0.0.53%lo:53", "127.0.0.53%lo", 53),
            // Older ss versions print IPv6 without brackets
            (":::22", "::", 22),
        ] {
            assert_eq!(
                split_listen_address(local),
                Some((address.to_string(), port)),
                "{local}"
            );
        }
        for bad in ["[::1]", "[::1]:http", "3000", ""] {
            assert_eq!(split_listen_address(bad), None, "{bad}");
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_parse_lsof_line_keeps_the_full_name() {
        // With `+c 0` lsof prints the whole name, escaping spaces as \x20; the
        // default 9-character column used to turn python3.14 into "python3.1"
        let line = "Activity\\x20Monitor  644  zee  8u  IPv4 0x3cc8c443bf3e8f62  0t0  TCP 127.0.0.1:7768 (LISTEN)";
        let info = PortInfo::parse_lsof_line(line).unwrap();
        assert_eq!(info.process_name, "Activity Monitor");
        assert_eq!(info.pid, 644);
        assert_eq!(info.port, 7768);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_parse_lsof_line_ipv6_has_no_brackets() {
        // lsof brackets IPv6 addresses, as ss and netstat do; the address is
        // reported without them on every platform, so --local matches "::1"
        let line =
            "python3.12  4242  zee  3u  IPv6 0x3cc8c443bf3e8f62  0t0  TCP [::1]:8000 (LISTEN)";
        let info = PortInfo::parse_lsof_line(line).unwrap();
        assert_eq!(info.address.as_deref(), Some("::1"));
        assert_eq!(info.port, 8000);
        assert_eq!(info.pid, 4242);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_ss_line_keeps_the_whole_ipv6_address() {
        // The address used to be the text between the last two colons, so
        // [::1]:39081 came back as "1]" and [::]:80 as "]"
        let loopback = r#"LISTEN 0      128            [::1]:39081         [::]:*    users:(("python3",pid=2897694,fd=3))"#;
        let info = PortInfo::parse_ss_line(loopback).unwrap();
        assert_eq!(info.address.as_deref(), Some("::1"));
        assert_eq!(info.port, 39081);
        assert_eq!(info.pid, 2897694);
        assert_eq!(info.process_name, "python3");

        let any = r#"LISTEN 0      511               [::]:80            [::]:*    users:(("nginx",pid=1201,fd=7))"#;
        let info = PortInfo::parse_ss_line(any).unwrap();
        assert_eq!(info.address.as_deref(), Some("::"));
        assert_eq!(info.port, 80);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_ss_line_ipv4_address() {
        let loopback = r#"LISTEN 0      128        127.0.0.1:43171      0.0.0.0:*    users:(("python3",pid=2897694,fd=4))"#;
        let info = PortInfo::parse_ss_line(loopback).unwrap();
        assert_eq!(info.address.as_deref(), Some("127.0.0.1"));
        assert_eq!(info.port, 43171);

        // `*` is every address, IPv4 and IPv6
        let any = r#"LISTEN 0      4096               *:8080             *:*    users:(("node",pid=4242,fd=21))"#;
        let info = PortInfo::parse_ss_line(any).unwrap();
        assert_eq!(info.address.as_deref(), Some("0.0.0.0"));
        assert_eq!(info.port, 8080);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_parse_netstat_line_keeps_the_whole_ipv6_address() {
        let line = "  TCP    [::]:135               [::]:0                 LISTENING       1092";
        let info = PortInfo::parse_netstat_line(line).unwrap();
        assert_eq!(info.address.as_deref(), Some("::"));
        assert_eq!(info.port, 135);
        assert_eq!(info.pid, 1092);
    }

    #[test]
    fn test_get_listening_ports() {
        // This test may or may not find ports depending on the system
        let result = PortInfo::get_all_listening();
        assert!(result.is_ok());
    }
}
