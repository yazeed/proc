//! Port discovery and management
//!
//! Provides cross-platform utilities for discovering which processes
//! are listening on network ports.

use crate::core::{escape_control, Process};
use crate::error::{ProcError, Result};
use serde::{Deserialize, Serialize};
// lsof on macOS, netstat and tasklist on Windows; Linux reads /proc instead
#[cfg(not(target_os = "linux"))]
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
    /// \xNN, so "Activity Monitor" arrives as "Activity\x20Monitor".
    ///
    /// Only printable ASCII is decoded. Control characters stay escaped, so a
    /// process name can't send escape sequences to the terminal, and the name is
    /// read as bytes, so a "\x" before a multi-byte character can't panic.
    #[cfg(target_os = "macos")]
    fn unescape_lsof(name: &str) -> String {
        fn hex(b: u8) -> Option<u8> {
            (b as char).to_digit(16).map(|d| d as u8)
        }
        let bytes = name.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if let [b'\\', b'x', hi, lo, ..] = bytes[i..] {
                if let (Some(hi), Some(lo)) = (hex(hi), hex(lo)) {
                    let decoded = hi << 4 | lo;
                    if (0x20..=0x7e).contains(&decoded) {
                        out.push(decoded);
                        i += 4;
                        continue;
                    }
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

        let process_name = escape_control(&Self::unescape_lsof(parts[0])).into_owned();
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

    /// Linux: read the kernel's socket tables directly, as `ss` does, so nothing
    /// needs to be installed. Listening sockets come from /proc/net/tcp and tcp6,
    /// and each socket's process is the one with that socket open in /proc/<pid>/fd.
    /// Sockets of other users' processes have no visible owner without root, so
    /// they are left out, as with `ss -p`.
    #[cfg(target_os = "linux")]
    fn get_listening_linux() -> Result<Vec<PortInfo>> {
        let mut sockets = Vec::new();
        let mut read_any = false;
        for (table, v6) in [("/proc/net/tcp", false), ("/proc/net/tcp6", true)] {
            // tcp6 is missing when IPv6 is disabled
            let Ok(text) = std::fs::read_to_string(table) else {
                continue;
            };
            read_any = true;
            sockets.extend(
                text.lines()
                    .skip(1)
                    .filter_map(|line| Self::parse_proc_net_line(line, v6)),
            );
        }
        if !read_any {
            return Err(ProcError::SystemError(
                "Can't read /proc/net/tcp to look up ports".to_string(),
            ));
        }

        let wanted: std::collections::HashSet<u64> = sockets.iter().map(|s| s.0).collect();
        let owners = Self::socket_owners(&wanted);

        let mut ports = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (inode, port, address) in sockets {
            let Some(&pid) = owners.get(&inode) else {
                continue;
            };
            // Same port on IPv4 and IPv6 is listed once, as on macOS
            if !seen.insert((port, pid)) {
                continue;
            }
            // any process can set its own comm (prctl), escape sequences included
            let process_name = std::fs::read_to_string(format!("/proc/{pid}/comm"))
                .map(|name| {
                    let name = name.strip_suffix('\n').unwrap_or(&name);
                    escape_control(name).into_owned()
                })
                .unwrap_or_else(|_| "unknown".to_string());
            ports.push(PortInfo {
                port,
                protocol: Protocol::Tcp,
                pid,
                process_name,
                address: Some(address),
            });
        }
        Ok(ports)
    }

    /// Parse a line of /proc/net/tcp or tcp6 into (socket inode, port, address),
    /// keeping only listening sockets. Addresses are hex in the kernel's byte
    /// order: 0100007F:1F90 is 127.0.0.1:8080.
    #[cfg(target_os = "linux")]
    fn parse_proc_net_line(line: &str, v6: bool) -> Option<(u64, u16, String)> {
        const LISTEN: &str = "0A";
        let fields: Vec<&str> = line.split_whitespace().collect();
        let (local, state, inode) = (fields.get(1)?, fields.get(3)?, fields.get(9)?);
        if *state != LISTEN {
            return None;
        }
        let (addr_hex, port_hex) = local.split_once(':')?;
        let port = u16::from_str_radix(port_hex, 16).ok()?;
        let inode: u64 = inode.parse().ok()?;

        // Each 32-bit word is printed as a number in host byte order
        let word = |i: usize| -> Option<[u8; 4]> {
            let hex = addr_hex.get(i * 8..i * 8 + 8)?;
            Some(u32::from_str_radix(hex, 16).ok()?.to_ne_bytes())
        };
        let address = if v6 {
            if addr_hex.len() != 32 {
                return None;
            }
            let mut bytes = [0u8; 16];
            for i in 0..4 {
                bytes[i * 4..i * 4 + 4].copy_from_slice(&word(i)?);
            }
            std::net::Ipv6Addr::from(bytes).to_string()
        } else {
            if addr_hex.len() != 8 {
                return None;
            }
            std::net::Ipv4Addr::from(word(0)?).to_string()
        };
        Some((inode, port, address))
    }

    /// Map socket inodes to the PID that has them open, by reading the
    /// "socket:[inode]" links in /proc/<pid>/fd. Unreadable processes are skipped.
    #[cfg(target_os = "linux")]
    fn socket_owners(
        wanted: &std::collections::HashSet<u64>,
    ) -> std::collections::HashMap<u64, u32> {
        let mut owners = std::collections::HashMap::new();
        let Ok(procs) = std::fs::read_dir("/proc") else {
            return owners;
        };
        for entry in procs.flatten() {
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|s| s.parse::<u32>().ok())
            else {
                continue;
            };
            let Ok(fds) = std::fs::read_dir(entry.path().join("fd")) else {
                continue;
            };
            for fd in fds.flatten() {
                let Ok(target) = std::fs::read_link(fd.path()) else {
                    continue;
                };
                let inode = target
                    .to_str()
                    .and_then(|t| t.strip_prefix("socket:["))
                    .and_then(|t| t.strip_suffix(']'))
                    .and_then(|t| t.parse::<u64>().ok());
                if let Some(inode) = inode.filter(|i| wanted.contains(i)) {
                    // the first process seen keeps it; a forked child sharing
                    // the socket usually comes later in PID order
                    owners.entry(inode).or_insert(pid);
                }
            }
        }
        owners
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
        Some(escape_control(name.trim_matches('"')).into_owned())
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
    fn test_unescape_lsof_is_safe_for_any_name() {
        // Control characters stay escaped, so a process name can't inject
        // terminal escape sequences (ESC [ 2 J clears the screen)
        assert_eq!(PortInfo::unescape_lsof("evil\\x1b[2J"), "evil\\x1b[2J");
        assert_eq!(PortInfo::unescape_lsof("a\\x7fb"), "a\\x7fb");
        assert_eq!(
            PortInfo::unescape_lsof("Activity\\x20Monitor"),
            "Activity Monitor"
        );
        // "\x" followed by a multi-byte character used to panic on a byte slice
        assert_eq!(PortInfo::unescape_lsof("x\\xé1"), "x\\xé1");
        assert_eq!(PortInfo::unescape_lsof("x\\x"), "x\\x");
        assert_eq!(PortInfo::unescape_lsof("x\\x4"), "x\\x4");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_proc_net_tcp_lines() {
        // 127.0.0.1:8080 listening (state 0A), socket inode 123456
        let v4 = "   0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 123456 1 0000000000000000 100 0 0 10 0";
        assert_eq!(
            PortInfo::parse_proc_net_line(v4, false),
            Some((123456, 8080, "127.0.0.1".to_string()))
        );
        // 0.0.0.0:22
        let any = "   1: 00000000:0016 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 777 1 0000000000000000 100 0 0 10 0";
        assert_eq!(
            PortInfo::parse_proc_net_line(any, false),
            Some((777, 22, "0.0.0.0".to_string()))
        );
        // An established connection (state 01) is not a listening port
        let established = "   2: 0100007F:1F90 0100007F:D431 01 00000000:00000000 00:00000000 00000000  1000        0 888 1 0000000000000000 20 4 30 10 -1";
        assert_eq!(PortInfo::parse_proc_net_line(established, false), None);
        // [::]:80 and [::1]:3000 from /proc/net/tcp6
        let v6_any = "   0: 00000000000000000000000000000000:0050 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 222 1 0000000000000000 100 0 0 10 0";
        assert_eq!(
            PortInfo::parse_proc_net_line(v6_any, true),
            Some((222, 80, "::".to_string()))
        );
        let v6_local = "   1: 00000000000000000000000001000000:0BB8 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 333 1 0000000000000000 100 0 0 10 0";
        assert_eq!(
            PortInfo::parse_proc_net_line(v6_local, true),
            Some((333, 3000, "::1".to_string()))
        );
        // Malformed lines are skipped, never a panic
        assert_eq!(PortInfo::parse_proc_net_line("garbage", false), None);
        assert_eq!(
            PortInfo::parse_proc_net_line("  0: ZZ:1F90 0:0 0A", false),
            None
        );
    }

    #[test]
    fn test_get_listening_ports() {
        // This test may or may not find ports depending on the system
        let result = PortInfo::get_all_listening();
        assert!(result.is_ok());
    }
}
