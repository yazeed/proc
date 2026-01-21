//! Cross-platform process abstraction
//!
//! Provides a unified interface for discovering and managing processes
//! across macOS, Linux, and Windows.

use crate::error::{ProcError, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use sysinfo::{Pid, ProcessStatus as SysProcessStatus, System};

/// Process status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProcessStatus {
    /// Process is actively executing on CPU
    Running,
    /// Process is waiting for an event or resource
    Sleeping,
    /// Process has been stopped (e.g., by SIGSTOP)
    Stopped,
    /// Process has terminated but not yet been reaped by parent
    Zombie,
    /// Process is being terminated
    Dead,
    /// Process status could not be determined
    Unknown,
}

impl From<SysProcessStatus> for ProcessStatus {
    fn from(status: SysProcessStatus) -> Self {
        match status {
            SysProcessStatus::Run => ProcessStatus::Running,
            SysProcessStatus::Sleep => ProcessStatus::Sleeping,
            SysProcessStatus::Stop => ProcessStatus::Stopped,
            SysProcessStatus::Zombie => ProcessStatus::Zombie,
            SysProcessStatus::Dead => ProcessStatus::Dead,
            _ => ProcessStatus::Unknown,
        }
    }
}

/// Represents a system process with relevant information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Process {
    /// Process ID
    pub pid: u32,
    /// Process name (executable name)
    pub name: String,
    /// Path to the executable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exe_path: Option<String>,
    /// Current working directory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Full command line (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// CPU usage percentage (0.0 - 100.0+)
    pub cpu_percent: f32,
    /// Memory usage in megabytes
    pub memory_mb: f64,
    /// Process status
    pub status: ProcessStatus,
    /// User who owns the process
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Parent process ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_pid: Option<u32>,
    /// Process start time (Unix timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<u64>,
}

impl Process {
    /// Find all processes matching a name pattern (case-insensitive)
    pub fn find_by_name(pattern: &str) -> Result<Vec<Process>> {
        let mut sys = System::new_all();
        sys.refresh_all();

        let pattern_lower = pattern.to_lowercase();
        let processes: Vec<Process> = sys
            .processes()
            .iter()
            .filter_map(|(pid, proc)| {
                let name = proc.name().to_string_lossy().to_string();
                let cmd: String = proc
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ");

                // Match against name or command
                if name.to_lowercase().contains(&pattern_lower)
                    || cmd.to_lowercase().contains(&pattern_lower)
                {
                    Some(Process::from_sysinfo(*pid, proc))
                } else {
                    None
                }
            })
            .collect();

        if processes.is_empty() {
            return Err(ProcError::ProcessNotFound(pattern.to_string()));
        }

        Ok(processes)
    }

    /// Find a specific process by PID
    pub fn find_by_pid(pid: u32) -> Result<Option<Process>> {
        let mut sys = System::new_all();
        sys.refresh_all();

        let sysinfo_pid = Pid::from_u32(pid);

        Ok(sys
            .processes()
            .get(&sysinfo_pid)
            .map(|proc| Process::from_sysinfo(sysinfo_pid, proc)))
    }

    /// Get all running processes
    pub fn find_all() -> Result<Vec<Process>> {
        let mut sys = System::new_all();
        sys.refresh_all();

        let processes: Vec<Process> = sys
            .processes()
            .iter()
            .map(|(pid, proc)| Process::from_sysinfo(*pid, proc))
            .collect();

        Ok(processes)
    }

    /// Find processes that appear to be stuck (high CPU, no progress)
    /// This is a heuristic-based detection
    pub fn find_stuck(timeout: Duration) -> Result<Vec<Process>> {
        let mut sys = System::new_all();
        sys.refresh_all();

        // Wait a bit and refresh to compare
        std::thread::sleep(Duration::from_millis(500));
        sys.refresh_all();

        let timeout_secs = timeout.as_secs();
        let processes: Vec<Process> = sys
            .processes()
            .iter()
            .filter_map(|(pid, proc)| {
                let cpu = proc.cpu_usage();
                let run_time = proc.run_time();

                // Heuristic: Process using significant CPU for longer than timeout
                // and in a potentially stuck state
                if run_time > timeout_secs && cpu > 50.0 {
                    Some(Process::from_sysinfo(*pid, proc))
                } else {
                    None
                }
            })
            .collect();

        Ok(processes)
    }

    /// Force kill the process (SIGKILL on Unix, taskkill /F on Windows)
    pub fn kill(&self) -> Result<()> {
        let mut sys = System::new();
        sys.refresh_processes(
            sysinfo::ProcessesToUpdate::Some(&[Pid::from_u32(self.pid)]),
            true,
        );

        if let Some(proc) = sys.process(Pid::from_u32(self.pid)) {
            if proc.kill() {
                Ok(())
            } else {
                Err(ProcError::SignalError(format!(
                    "Failed to kill process {}",
                    self.pid
                )))
            }
        } else {
            Err(ProcError::ProcessNotFound(self.pid.to_string()))
        }
    }

    /// Force kill and wait for process to terminate
    /// Returns the exit status if available
    pub fn kill_and_wait(&self) -> Result<Option<std::process::ExitStatus>> {
        let mut sys = System::new();
        sys.refresh_processes(
            sysinfo::ProcessesToUpdate::Some(&[Pid::from_u32(self.pid)]),
            true,
        );

        if let Some(proc) = sys.process(Pid::from_u32(self.pid)) {
            proc.kill_and_wait().map_err(|e| {
                ProcError::SignalError(format!("Failed to kill process {}: {:?}", self.pid, e))
            })
        } else {
            Err(ProcError::ProcessNotFound(self.pid.to_string()))
        }
    }

    /// Send SIGTERM for graceful termination (Unix) or taskkill (Windows)
    #[cfg(unix)]
    pub fn terminate(&self) -> Result<()> {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid as NixPid;

        kill(NixPid::from_raw(self.pid as i32), Signal::SIGTERM)
            .map_err(|e| ProcError::SignalError(e.to_string()))
    }

    /// Graceful termination (Windows)
    #[cfg(windows)]
    pub fn terminate(&self) -> Result<()> {
        use std::process::Command;

        Command::new("taskkill")
            .args(["/PID", &self.pid.to_string()])
            .output()
            .map_err(|e| ProcError::SystemError(e.to_string()))?;

        Ok(())
    }

    /// Check if the process still exists
    pub fn exists(&self) -> bool {
        let mut sys = System::new();
        sys.refresh_processes(
            sysinfo::ProcessesToUpdate::Some(&[Pid::from_u32(self.pid)]),
            true,
        );
        sys.process(Pid::from_u32(self.pid)).is_some()
    }

    /// Check if the process is still running (alias for exists for compatibility)
    pub fn is_running(&self) -> bool {
        self.exists()
    }

    /// Wait for the process to terminate
    /// Returns the exit status if available
    pub fn wait(&self) -> Option<std::process::ExitStatus> {
        let mut sys = System::new();
        sys.refresh_processes(
            sysinfo::ProcessesToUpdate::Some(&[Pid::from_u32(self.pid)]),
            true,
        );

        sys.process(Pid::from_u32(self.pid))
            .and_then(|proc| proc.wait())
    }

    /// Convert from sysinfo Process
    fn from_sysinfo(pid: Pid, proc: &sysinfo::Process) -> Self {
        let cmd_vec = proc.cmd();
        let command = if cmd_vec.is_empty() {
            None
        } else {
            Some(
                cmd_vec
                    .iter()
                    .map(|s| s.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        };

        let exe_path = proc.exe().map(|p| p.to_string_lossy().to_string());
        let cwd = proc.cwd().map(|p| p.to_string_lossy().to_string());

        Process {
            pid: pid.as_u32(),
            name: proc.name().to_string_lossy().to_string(),
            exe_path,
            cwd,
            command,
            cpu_percent: proc.cpu_usage(),
            memory_mb: proc.memory() as f64 / 1024.0 / 1024.0,
            status: ProcessStatus::from(proc.status()),
            user: proc.user_id().map(|u| u.to_string()),
            parent_pid: proc.parent().map(|p| p.as_u32()),
            start_time: Some(proc.start_time()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_all_processes() {
        let processes = Process::find_all().unwrap();
        assert!(!processes.is_empty(), "Should find at least one process");
    }

    #[test]
    fn test_find_by_pid_self() {
        let pid = std::process::id();
        let process = Process::find_by_pid(pid).unwrap();
        assert!(process.is_some(), "Should find own process");
    }

    #[test]
    fn test_find_nonexistent_process() {
        let result = Process::find_by_name("nonexistent_process_12345");
        assert!(result.is_err());
    }
}
