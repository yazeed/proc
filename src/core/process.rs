//! Cross-platform process abstraction
//!
//! Provides a unified interface for discovering and managing processes
//! across macOS, Linux, and Windows.

use crate::error::{ProcError, Result};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::Duration;
use sysinfo::{Pid, ProcessStatus as SysProcessStatus, System, Uid, Users};

/// Resolve a user ID to its user name, falling back to the ID itself.
/// The user list is read once per run.
fn user_name(uid: &Uid) -> String {
    static USERS: OnceLock<Users> = OnceLock::new();
    USERS
        .get_or_init(Users::new_with_refreshed_list)
        .get_user_by_id(uid)
        .map(|user| user.name().to_string())
        .unwrap_or_else(|| uid.to_string())
}

/// macOS: sysinfo drops zombies, because they have no executable path or arguments
/// left to read, so `proc list --status zombie` could never find one. Read every
/// process from the kernel, as `ps` does, and keep the zombies sysinfo didn't return.
#[cfg(target_os = "macos")]
fn macos_zombies(known: &std::collections::HashSet<u32>) -> Vec<Process> {
    // libc has no kinfo_proc for macOS, so read the few fields we need at their
    // offsets in the 64-bit layout (checked against a live process's known values)
    const KINFO_PROC_SIZE: usize = 648;
    const P_STARTTIME_SEC: usize = 0;
    const P_STAT: usize = 36;
    const P_PID: usize = 40;
    const P_COMM: usize = 243; // MAXCOMLEN + 1 bytes
    const P_RUID: usize = 392;
    const E_PPID: usize = 560;
    const SZOMB: u8 = 5;

    let mut mib = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_ALL, 0];
    let mut size: libc::size_t = 0;
    let mut buf: Vec<u8> = Vec::new();
    // SAFETY: the first call only reports the size needed; the second fills a
    // buffer of the size we pass, with room for processes started in between
    unsafe {
        let null = std::ptr::null_mut();
        if libc::sysctl(mib.as_mut_ptr(), 4, null, &mut size, null, 0) != 0 {
            return Vec::new();
        }
        size += 64 * KINFO_PROC_SIZE;
        buf.resize(size, 0);
        if libc::sysctl(
            mib.as_mut_ptr(),
            4,
            buf.as_mut_ptr().cast(),
            &mut size,
            null,
            0,
        ) != 0
        {
            return Vec::new();
        }
    }
    buf.truncate(size);
    if !buf.len().is_multiple_of(KINFO_PROC_SIZE) {
        return Vec::new(); // a layout we don't know; better to show nothing than garbage
    }

    let read_u32 = |p: &[u8], at: usize| u32::from_ne_bytes(p[at..at + 4].try_into().unwrap());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    buf.as_chunks::<KINFO_PROC_SIZE>()
        .0
        .iter()
        .filter(|p| p[P_STAT] == SZOMB)
        .filter_map(|p| {
            let pid = read_u32(p, P_PID);
            if pid == 0 || known.contains(&pid) {
                return None;
            }
            let comm = &p[P_COMM..P_COMM + 17];
            let comm = &comm[..comm.iter().position(|&c| c == 0).unwrap_or(comm.len())];
            let name = String::from_utf8_lossy(comm).into_owned();
            let start =
                u64::from_ne_bytes(p[P_STARTTIME_SEC..P_STARTTIME_SEC + 8].try_into().unwrap());
            Some(Process {
                pid,
                name: if name.is_empty() {
                    "<defunct>".to_string()
                } else {
                    name
                },
                exe_path: None,
                cwd: None,
                command: None,
                cpu_percent: 0.0,
                memory_mb: 0.0,
                status: ProcessStatus::Zombie,
                user: Uid::try_from(read_u32(p, P_RUID) as usize)
                    .ok()
                    .map(|uid| user_name(&uid)),
                parent_pid: Some(read_u32(p, E_PPID)),
                start_time: (start > 0).then_some(start),
                uptime_secs: (start > 0).then(|| now.saturating_sub(start)),
            })
        })
        .collect()
}

/// On Linux, sysinfo lists every thread of a process as an entry of its own,
/// with the process's command line, so a node server shows up a dozen times.
/// Process listings skip them; `thread_kind` is always `None` elsewhere.
pub fn is_thread(proc: &sysinfo::Process) -> bool {
    proc.thread_kind().is_some()
}

/// Linux reports a working directory that was deleted as "/path (deleted)".
/// Strip that marker so the cwd matches `--in` and reads the same as on macOS;
/// a real directory with that name still exists, so it is kept as is.
fn strip_deleted_marker(cwd: String) -> String {
    match cwd.strip_suffix(" (deleted)") {
        Some(path) if !std::path::Path::new(&cwd).exists() => path.to_string(),
        _ => cwd,
    }
}

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
    /// Seconds the process has been running
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime_secs: Option<u64>,
}

impl Process {
    /// Find all processes matching a name pattern (case-insensitive)
    pub fn find_by_name(pattern: &str) -> Result<Vec<Process>> {
        let mut sys = System::new_all();
        sys.refresh_all();

        let pattern_lower = pattern.to_lowercase();
        let self_pid = sysinfo::Pid::from_u32(std::process::id());
        // Exclude the entire ancestor chain — their command lines contain proc's
        // arguments (e.g. "zsh -c proc by node"), causing false positives.
        // Walk up to 10 levels (covers: proc → shell → shell wrapper → IDE → ...)
        let ancestor_pids = {
            let mut pids = std::collections::HashSet::new();
            pids.insert(self_pid);
            let mut current = self_pid;
            for _ in 0..10 {
                if let Some(parent) = sys.process(current).and_then(|p| p.parent()) {
                    pids.insert(parent);
                    current = parent;
                } else {
                    break;
                }
            }
            pids
        };
        let processes: Vec<Process> = sys
            .processes()
            .iter()
            .filter_map(|(pid, proc)| {
                // Exclude self, ancestor shells, and Linux threads
                if ancestor_pids.contains(pid) || is_thread(proc) {
                    return None;
                }

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

    /// Get all running processes (excludes proc's own process)
    pub fn find_all() -> Result<Vec<Process>> {
        let mut sys = System::new_all();
        sys.refresh_all();

        let self_pid = sysinfo::Pid::from_u32(std::process::id());
        #[allow(unused_mut)]
        let mut processes: Vec<Process> = sys
            .processes()
            .iter()
            .filter(|(pid, proc)| **pid != self_pid && !is_thread(proc))
            .map(|(pid, proc)| Process::from_sysinfo(*pid, proc))
            .collect();

        #[cfg(target_os = "macos")]
        {
            let known: std::collections::HashSet<u32> = processes.iter().map(|p| p.pid).collect();
            processes.extend(macos_zombies(&known));
        }

        Ok(processes)
    }

    /// Find processes running a specific executable path
    pub fn find_by_exe_path(path: &std::path::Path) -> Result<Vec<Process>> {
        let all = Self::find_all()?;
        let path_str = path.to_string_lossy();

        Ok(all
            .into_iter()
            .filter(|p| {
                if let Some(ref exe) = p.exe_path {
                    // Exact match (compare as strings to avoid PathBuf allocation)
                    exe == &*path_str || std::path::Path::new(exe) == path
                } else {
                    false
                }
            })
            .collect())
    }

    /// Find processes that have a file open (Unix only via lsof)
    #[cfg(unix)]
    pub fn find_by_open_file(path: &std::path::Path) -> Result<Vec<Process>> {
        use std::process::Command;

        let output = Command::new("lsof")
            .args(["-t", &path.to_string_lossy()]) // -t = terse (PIDs only)
            .output();

        let output = match output {
            Ok(o) => o,
            Err(_) => return Ok(vec![]), // lsof not available
        };

        if !output.status.success() {
            return Ok(vec![]); // No processes have file open
        }

        let pids: Vec<u32> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| line.trim().parse().ok())
            .collect();

        let mut processes = Vec::new();
        for pid in pids {
            if let Ok(Some(proc)) = Self::find_by_pid(pid) {
                processes.push(proc);
            }
        }

        Ok(processes)
    }

    /// Find processes that have a file open (Windows stub)
    #[cfg(not(unix))]
    pub fn find_by_open_file(_path: &std::path::Path) -> Result<Vec<Process>> {
        // Windows: Could use handle.exe from Sysinternals, but skip for now
        Ok(vec![])
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
                if is_thread(proc) {
                    return None;
                }
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

        match sys.process(Pid::from_u32(self.pid)) {
            Some(proc) if self.is_same(proc) => {
                if proc.kill() {
                    Ok(())
                } else {
                    Err(ProcError::SignalError(format!(
                        "Failed to kill process {}",
                        self.pid
                    )))
                }
            }
            // gone, or the PID now belongs to a new process
            _ => Err(ProcError::ProcessNotFound(self.pid.to_string())),
        }
    }

    /// Whether `current`, the process now holding this PID, is the one this record
    /// describes. A different start time means the PID was reused by a new process.
    fn is_same(&self, current: &sysinfo::Process) -> bool {
        match self.start_time {
            Some(ours) if current.start_time() > 0 => current.start_time().abs_diff(ours) <= 1,
            _ => true,
        }
    }

    /// Checked right before each signal, so a delayed one (stop's SIGKILL comes up
    /// to --timeout seconds after the target was resolved) can't reach a new
    /// process that reused the PID.
    fn ensure_same_process(&self) -> Result<()> {
        let mut sys = System::new();
        let pid = Pid::from_u32(self.pid);
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
        match sys.process(pid) {
            Some(current) if self.is_same(current) => Ok(()),
            _ => Err(ProcError::ProcessNotFound(self.pid.to_string())),
        }
    }

    /// Force kill and wait up to 5 seconds for the process to be gone.
    ///
    /// Never blocks forever (sysinfo's own kill_and_wait polls with no limit and
    /// never ends for a zombie). A process still alive after 5 seconds, such as
    /// one stuck in the kernel, is an error.
    pub fn kill_and_wait(&self) -> Result<()> {
        match self.kill() {
            // it exited on its own just before the kill
            Err(ProcError::ProcessNotFound(_)) => return Ok(()),
            other => other?,
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if !self.is_running() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Err(ProcError::SignalError(format!(
            "Process {} is still running 5 seconds after SIGKILL",
            self.pid
        )))
    }

    /// Send SIGTERM for graceful termination (Unix) or taskkill (Windows)
    #[cfg(unix)]
    pub fn terminate(&self) -> Result<()> {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid as NixPid;

        self.ensure_same_process()?;
        kill(NixPid::from_raw(self.pid as i32), Signal::SIGTERM)
            .map_err(|e| ProcError::SignalError(e.to_string()))
    }

    /// Graceful termination (Windows)
    #[cfg(windows)]
    pub fn terminate(&self) -> Result<()> {
        use std::process::Command;

        self.ensure_same_process()?;
        Command::new("taskkill")
            .args(["/PID", &self.pid.to_string()])
            .output()
            .map_err(|e| ProcError::SystemError(e.to_string()))?;

        Ok(())
    }

    /// Send an arbitrary signal to the process (Unix only)
    #[cfg(unix)]
    pub fn send_signal(&self, signal: nix::sys::signal::Signal) -> Result<()> {
        use nix::sys::signal::kill;
        use nix::unistd::Pid as NixPid;
        self.ensure_same_process()?;
        kill(NixPid::from_raw(self.pid as i32), signal)
            .map_err(|e| ProcError::SignalError(format!("{}: {}", signal, e)))
    }

    // Note: On Windows, send_signal is not available. Commands that use it
    // (freeze, thaw) have their own #[cfg(not(unix))] stubs that return NotSupported.

    /// Find orphaned processes (parent is PID 1 / init / launchd, excluding system daemons)
    pub fn find_orphans() -> Result<Vec<Process>> {
        let all = Self::find_all()?;

        Ok(all
            .into_iter()
            .filter(|p| {
                if let Some(ppid) = p.parent_pid {
                    ppid == 1 && p.pid != 1 && !Self::is_system_process(p)
                } else {
                    false
                }
            })
            .collect())
    }

    /// Check if a process is a system daemon (naturally has PPID 1)
    ///
    /// Filters out processes that legitimately have PPID 1 — system daemons,
    /// kernel threads, and services managed by init/systemd/launchd.
    /// Heuristic: no cwd or cwd is "/" with exe in system paths.
    fn is_system_process(p: &Process) -> bool {
        if p.cwd.is_none() || p.cwd.as_deref() == Some("/") {
            if let Some(ref exe) = p.exe_path {
                // macOS system paths
                if exe.starts_with("/System/") || exe.starts_with("/usr/libexec/") {
                    return true;
                }
                // Shared Unix system paths (macOS + Linux)
                if exe.starts_with("/usr/sbin/")
                    || exe.starts_with("/sbin/")
                    || exe.starts_with("/usr/bin/")
                    || exe.starts_with("/usr/lib/")
                    || exe.starts_with("/usr/lib64/")
                    || exe.starts_with("/lib/")
                    || exe.starts_with("/lib64/")
                    || exe.starts_with("/opt/")
                    || exe.starts_with("/snap/")
                {
                    return true;
                }
            }
            return true; // No exe path = likely kernel thread
        }
        false
    }

    /// Check if the process is still alive. A zombie (exited, not yet reaped by
    /// its parent) has exited, and a process with a different start time is a new
    /// one that reused the PID, so neither counts.
    pub fn exists(&self) -> bool {
        let mut sys = System::new();
        let pid = Pid::from_u32(self.pid);
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
        match sys.process(pid) {
            None => false,
            Some(current) if current.status() == SysProcessStatus::Zombie => false,
            Some(current) => self.is_same(current),
        }
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
    pub(crate) fn from_sysinfo(pid: Pid, proc: &sysinfo::Process) -> Self {
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
        let cwd = proc
            .cwd()
            .map(|p| strip_deleted_marker(p.to_string_lossy().to_string()));

        Process {
            pid: pid.as_u32(),
            name: proc.name().to_string_lossy().to_string(),
            exe_path,
            cwd,
            command,
            cpu_percent: proc.cpu_usage(),
            memory_mb: proc.memory() as f64 / 1024.0 / 1024.0,
            status: ProcessStatus::from(proc.status()),
            user: proc.user_id().map(user_name),
            parent_pid: proc.parent().map(|p| p.as_u32()),
            // sysinfo reports 0 when it can't read the start time (on macOS, any
            // other user's process), which would make the uptime about 56 years
            start_time: (proc.start_time() > 0).then(|| proc.start_time()),
            uptime_secs: (proc.start_time() > 0).then(|| proc.run_time()),
        }
    }
}

/// Parse a signal name to a nix Signal (Unix only)
///
/// Accepts: "HUP", "SIGHUP", "hup" (signal names only, not numbers —
/// numeric signal values differ between macOS and Linux)
#[cfg(unix)]
pub fn parse_signal_name(name: &str) -> Result<nix::sys::signal::Signal> {
    use nix::sys::signal::Signal;

    let upper = name.to_uppercase();
    let upper = upper.trim_start_matches("SIG");
    match upper {
        "HUP" => Ok(Signal::SIGHUP),
        "INT" => Ok(Signal::SIGINT),
        "QUIT" => Ok(Signal::SIGQUIT),
        "ABRT" => Ok(Signal::SIGABRT),
        "KILL" => Ok(Signal::SIGKILL),
        "TERM" => Ok(Signal::SIGTERM),
        "STOP" => Ok(Signal::SIGSTOP),
        "CONT" => Ok(Signal::SIGCONT),
        "USR1" => Ok(Signal::SIGUSR1),
        "USR2" => Ok(Signal::SIGUSR2),
        _ => Err(ProcError::InvalidInput(format!(
            "Unknown signal: '{}'. Valid signals: HUP, INT, QUIT, ABRT, KILL, TERM, STOP, CONT, USR1, USR2",
            name
        ))),
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
    fn test_is_running_tells_a_reused_pid_apart() {
        let mut me = Process::find_by_pid(std::process::id()).unwrap().unwrap();
        assert!(me.is_running());
        // Same PID, different start time: a new process that reused the PID
        if let Some(start) = me.start_time {
            me.start_time = Some(start - 3600);
            assert!(!me.is_running(), "a reused PID counted as the same process");
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_signals_never_reach_a_process_that_reused_the_pid() {
        let mut child = std::process::Command::new("sleep")
            .arg("300")
            .spawn()
            .unwrap();
        let mut stale = Process::find_by_pid(child.id()).unwrap().unwrap();
        // A record from before the PID was reused: same PID, older start time
        stale.start_time = stale.start_time.map(|t| t - 3600);

        let kill = stale.kill();
        let term = stale.terminate();
        let sig = stale.send_signal(nix::sys::signal::Signal::SIGKILL);
        let alive = child.try_wait().unwrap().is_none();
        let _ = child.kill();
        let _ = child.wait();

        assert!(
            alive,
            "a stale record signalled the process now using its PID"
        );
        assert!(
            matches!(kill, Err(ProcError::ProcessNotFound(_))),
            "{kill:?}"
        );
        assert!(
            matches!(term, Err(ProcError::ProcessNotFound(_))),
            "{term:?}"
        );
        assert!(matches!(sig, Err(ProcError::ProcessNotFound(_))), "{sig:?}");
    }

    #[test]
    fn test_strip_deleted_marker() {
        assert_eq!(
            strip_deleted_marker("/nonexistent/proc-gone (deleted)".to_string()),
            "/nonexistent/proc-gone"
        );
        assert_eq!(
            strip_deleted_marker("/tmp/project".to_string()),
            "/tmp/project"
        );

        // A directory really named "x (deleted)" keeps its name
        let real = std::env::temp_dir().join(format!("proc-real-{} (deleted)", std::process::id()));
        std::fs::create_dir_all(&real).unwrap();
        let path = real.to_string_lossy().to_string();
        assert_eq!(strip_deleted_marker(path.clone()), path);
        std::fs::remove_dir(&real).unwrap();
    }

    #[test]
    fn test_find_nonexistent_process() {
        let result = Process::find_by_name("nonexistent_process_12345");
        assert!(result.is_err());
    }

    #[test]
    fn test_find_orphans_returns_ok() {
        // Should not error — may or may not find orphans depending on system state
        let result = Process::find_orphans();
        assert!(result.is_ok());
    }

    #[test]
    fn test_find_orphans_excludes_system_processes() {
        let orphans = Process::find_orphans().unwrap();
        for orphan in &orphans {
            // No orphan should have a system exe path with cwd "/"
            if orphan.cwd.as_deref() == Some("/") {
                if let Some(ref exe) = orphan.exe_path {
                    assert!(
                        !exe.starts_with("/usr/sbin/")
                            && !exe.starts_with("/sbin/")
                            && !exe.starts_with("/System/")
                            && !exe.starts_with("/usr/libexec/"),
                        "System process should have been filtered: {} ({})",
                        orphan.name,
                        exe
                    );
                }
            }
        }
    }

    #[test]
    fn test_is_system_process_system_paths() {
        let make_proc = |exe: Option<&str>, cwd: Option<&str>| Process {
            pid: 100,
            name: "test".to_string(),
            exe_path: exe.map(String::from),
            cwd: cwd.map(String::from),
            command: None,
            cpu_percent: 0.0,
            memory_mb: 0.0,
            status: ProcessStatus::Running,
            user: None,
            parent_pid: Some(1),
            start_time: None,
            uptime_secs: None,
        };

        // System daemons with cwd "/" and system exe paths
        assert!(Process::is_system_process(&make_proc(
            Some("/usr/sbin/sshd"),
            Some("/")
        )));
        assert!(Process::is_system_process(&make_proc(
            Some("/System/Library/foo"),
            Some("/")
        )));
        assert!(Process::is_system_process(&make_proc(
            Some("/usr/bin/systemd"),
            Some("/")
        )));
        assert!(Process::is_system_process(&make_proc(
            Some("/usr/lib/snapd/snapd"),
            Some("/")
        )));

        // No exe path with cwd "/" = kernel thread
        assert!(Process::is_system_process(&make_proc(None, Some("/"))));

        // No cwd at all = likely kernel thread
        assert!(Process::is_system_process(&make_proc(
            Some("/usr/bin/foo"),
            None
        )));

        // User process with user cwd = NOT system
        assert!(!Process::is_system_process(&make_proc(
            Some("/usr/bin/node"),
            Some("/home/user/project")
        )));
        assert!(!Process::is_system_process(&make_proc(
            Some("/home/user/.local/bin/app"),
            Some("/home/user")
        )));
    }

    #[cfg(unix)]
    #[test]
    fn test_parse_signal_name_valid() {
        use nix::sys::signal::Signal;

        assert_eq!(parse_signal_name("HUP").unwrap(), Signal::SIGHUP);
        assert_eq!(parse_signal_name("hup").unwrap(), Signal::SIGHUP);
        assert_eq!(parse_signal_name("SIGHUP").unwrap(), Signal::SIGHUP);
        assert_eq!(parse_signal_name("sighup").unwrap(), Signal::SIGHUP);
        assert_eq!(parse_signal_name("INT").unwrap(), Signal::SIGINT);
        assert_eq!(parse_signal_name("QUIT").unwrap(), Signal::SIGQUIT);
        assert_eq!(parse_signal_name("ABRT").unwrap(), Signal::SIGABRT);
        assert_eq!(parse_signal_name("KILL").unwrap(), Signal::SIGKILL);
        assert_eq!(parse_signal_name("TERM").unwrap(), Signal::SIGTERM);
        assert_eq!(parse_signal_name("STOP").unwrap(), Signal::SIGSTOP);
        assert_eq!(parse_signal_name("CONT").unwrap(), Signal::SIGCONT);
        assert_eq!(parse_signal_name("USR1").unwrap(), Signal::SIGUSR1);
        assert_eq!(parse_signal_name("USR2").unwrap(), Signal::SIGUSR2);
    }

    #[cfg(unix)]
    #[test]
    fn test_parse_signal_name_invalid() {
        assert!(parse_signal_name("INVALID").is_err());
        assert!(parse_signal_name("FOO").is_err());
        assert!(parse_signal_name("").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn test_parse_signal_name_case_insensitive() {
        use nix::sys::signal::Signal;

        assert_eq!(parse_signal_name("term").unwrap(), Signal::SIGTERM);
        assert_eq!(parse_signal_name("Term").unwrap(), Signal::SIGTERM);
        assert_eq!(parse_signal_name("TERM").unwrap(), Signal::SIGTERM);
        assert_eq!(parse_signal_name("sigterm").unwrap(), Signal::SIGTERM);
        assert_eq!(parse_signal_name("SigTerm").unwrap(), Signal::SIGTERM);
    }

    #[cfg(unix)]
    #[test]
    fn test_send_signal_nonexistent_process() {
        use nix::sys::signal::Signal;

        // PID 99999999 almost certainly doesn't exist
        let proc = Process {
            pid: 99999999,
            name: "ghost".to_string(),
            exe_path: None,
            cwd: None,
            command: None,
            cpu_percent: 0.0,
            memory_mb: 0.0,
            status: ProcessStatus::Running,
            user: None,
            parent_pid: None,
            start_time: None,
            uptime_secs: None,
        };

        let result = proc.send_signal(Signal::SIGCONT);
        assert!(result.is_err());
    }
}
