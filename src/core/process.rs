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

/// Make text from other processes safe to print: control characters become
/// visible escapes, as `lsof` and `ps` do. Names, command lines and paths are
/// set by whoever runs a process, and printed raw an ESC sequence could
/// recolour, retitle or rewrite the terminal of whoever runs proc (and a
/// newline would split a table row). Applied where process data is read, so
/// every view and the JSON show the same text.
///
/// Escaped: C0 controls and DEL (`\x1b`, with `\n`, `\r`, `\t` by name), C1
/// controls (`\x9b`), and bidirectional overrides that reorder text on screen
/// (`\u{202e}`). Everything else, non-ASCII included, is unchanged. Works on
/// chars, never byte offsets, so it can't split a character.
pub fn escape_control(text: &str) -> std::borrow::Cow<'_, str> {
    fn needs_escape(c: char) -> bool {
        c.is_control()
            || matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
    }
    if !text.chars().any(needs_escape) {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\x{:02x}", c as u32)),
            c if needs_escape(c) => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    std::borrow::Cow::Owned(out)
}

/// `stuck` and `unstick` take this many readings, this far apart, and a process
/// must look stuck in every one
const STUCK_READINGS: usize = 2;
const STUCK_READING_GAP: Duration = Duration::from_secs(1);

/// Stuck in one reading: busy (over half a core), or in uninterruptible sleep,
/// blocked in the kernel (usually on I/O) where it can't even be interrupted
fn looks_stuck(cpu_percent: f32, status: SysProcessStatus) -> bool {
    cpu_percent > 50.0 || status == SysProcessStatus::UninterruptibleDiskSleep
}

/// CPU use in percent of one core: the CPU time a process used between two
/// reads (`accumulated_cpu_time`, in ms) over the wall time between them.
/// sysinfo's own `cpu_usage` can't be trusted across two reads: on Windows a
/// process's first update stores no baseline, so the next one divides its
/// lifetime CPU by all CPU time since boot and reads ~0% for a full core.
fn cpu_percent_between(used_before_ms: u64, used_after_ms: u64, wall: Duration) -> f32 {
    let wall_ms = wall.as_secs_f64() * 1000.0;
    if wall_ms <= 0.0 {
        return 0.0;
    }
    (used_after_ms.saturating_sub(used_before_ms) as f64 / wall_ms * 100.0) as f32
}

/// Why a command such as `taskkill` failed, in its own words: what it wrote to
/// stderr, else to stdout, else its exit status
/// (compiled on Unix for the tests only)
#[cfg(any(windows, test))]
fn command_failure(stdout: &[u8], stderr: &[u8], status: &str) -> String {
    [stderr, stdout]
        .iter()
        .map(|out| {
            // one line, whatever the tool wrapped its reason over
            String::from_utf8_lossy(out)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .find(|text| !text.is_empty())
        .unwrap_or_else(|| format!("exited with {status}"))
}

/// The result of asking sysinfo to kill process `pid`, which says whether the
/// kill was delivered
fn kill_outcome(delivered: bool, pid: u32) -> Result<()> {
    if delivered {
        Ok(())
    } else {
        Err(ProcError::SignalError(format!(
            "Failed to kill process {}",
            pid
        )))
    }
}

/// What a `taskkill` run (no `/F`) came to: success, or the reason it gave
/// (compiled on Unix for the tests only)
#[cfg(any(windows, test))]
fn taskkill_outcome(output: &std::process::Output) -> Result<()> {
    if output.status.success() {
        Ok(())
    } else {
        Err(ProcError::SignalError(command_failure(
            &output.stdout,
            &output.stderr,
            &output.status.to_string(),
        )))
    }
}

/// The CPU time each process in `sys` has used so far, in ms
fn cpu_used(sys: &System) -> std::collections::HashMap<Pid, u64> {
    sys.processes()
        .iter()
        .map(|(pid, p)| (*pid, p.accumulated_cpu_time()))
        .collect()
}

/// Whether a process has been running for `timeout_secs`. A process's age is in whole
/// seconds, so a process under a second old reads 0 and must still count for
/// `--timeout 0`: "longer than" means this many seconds or more.
fn ran_long_enough(run_time: u64, timeout_secs: u64) -> bool {
    run_time >= timeout_secs
}

/// Whether one reading of a process counts toward "stuck": a process, not a
/// thread, that has run for `timeout_secs` and looks stuck. `find_stuck` filters
/// through this, so a test of it guards the comparison where it is used.
fn counts_as_stuck(is_thread: bool, run_time: u64, timeout_secs: u64, looks_stuck: bool) -> bool {
    !is_thread && ran_long_enough(run_time, timeout_secs) && looks_stuck
}

/// macOS: sysinfo drops zombies, because they have no executable path or arguments
/// left to read, so `proc list --status zombie` could never find one. Read every
/// process from the kernel, as `ps` does, and keep the zombies sysinfo didn't return.
#[cfg(target_os = "macos")]
fn macos_zombies(known: &std::collections::HashSet<u32>) -> Vec<Process> {
    let table = read_proc_table(|buf, size| {
        let mut mib = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_ALL, 0];
        let null = std::ptr::null_mut();
        // SAFETY: `buf` is null (report the size needed) or points to `*size`
        // writable bytes, which is how read_proc_table calls it
        unsafe { libc::sysctl(mib.as_mut_ptr(), 4, buf, size, null, 0) }
    });
    zombies_in(&table, known)
}

/// libc has no kinfo_proc for macOS, so the few fields we need are read at their
/// offsets in the 64-bit layout (checked against a live process's known values)
#[cfg(target_os = "macos")]
const KINFO_PROC_SIZE: usize = 648;

/// The kernel's table of every process, as raw kinfo_proc records; empty if the
/// kernel refuses. `sysctl` is the call to make (a buffer or null, and the size
/// in and out), so a test can make it fail.
#[cfg(target_os = "macos")]
fn read_proc_table(
    sysctl: impl Fn(*mut libc::c_void, &mut libc::size_t) -> libc::c_int,
) -> Vec<u8> {
    let mut size: libc::size_t = 0;
    // the first call only reports the size needed
    if sysctl(std::ptr::null_mut(), &mut size) != 0 {
        return Vec::new();
    }
    // room for processes started in between
    size += 64 * KINFO_PROC_SIZE;
    let mut buf: Vec<u8> = vec![0; size];
    if sysctl(buf.as_mut_ptr().cast(), &mut size) != 0 {
        return Vec::new();
    }
    buf.truncate(size);
    buf
}

/// The zombies in a table read by `read_proc_table` that `known` doesn't list
#[cfg(target_os = "macos")]
fn zombies_in(buf: &[u8], known: &std::collections::HashSet<u32>) -> Vec<Process> {
    const P_STARTTIME_SEC: usize = 0;
    const P_STAT: usize = 36;
    const P_PID: usize = 40;
    const P_COMM: usize = 243; // MAXCOMLEN + 1 bytes
    const P_RUID: usize = 392;
    const E_PPID: usize = 560;
    const SZOMB: u8 = 5;

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
            let name = escape_control(&String::from_utf8_lossy(comm)).into_owned();
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
                    .map(|uid| escape_control(&user_name(&uid)).into_owned()),
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
        Self::find_by_open_file_with("lsof", path)
    }

    /// `find_by_open_file` with the lsof program to run, so a test can have it missing
    #[cfg(unix)]
    fn find_by_open_file_with(lsof: &str, path: &std::path::Path) -> Result<Vec<Process>> {
        use std::process::Command;

        let output = Command::new(lsof)
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

    /// Find processes that look stuck: running longer than `timeout`, and in each
    /// of two readings a second apart either using over half a core or blocked in
    /// uninterruptible sleep. One reading
    /// used to be enough, so a momentary spike counted. A long build at full CPU
    /// still looks the same as a loop that never ends; only the user can tell.
    pub fn find_stuck(timeout: Duration) -> Result<Vec<Process>> {
        let timeout_secs = timeout.as_secs();
        // each read is timed from its start, as in `measure_cpu`
        let mut read_at = std::time::Instant::now();
        let mut sys = System::new_all();
        let mut used = cpu_used(&sys);
        let mut cpu: std::collections::HashMap<Pid, f32> = std::collections::HashMap::new();
        let mut stuck: Option<std::collections::HashSet<Pid>> = None;

        for _ in 0..STUCK_READINGS {
            std::thread::sleep(STUCK_READING_GAP);
            let started = std::time::Instant::now();
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            let wall = started - read_at;
            read_at = started;
            // a process first seen in this read has no CPU figure yet
            cpu = sys
                .processes()
                .iter()
                .filter_map(|(pid, p)| {
                    let before = used.get(pid)?;
                    Some((
                        *pid,
                        cpu_percent_between(*before, p.accumulated_cpu_time(), wall),
                    ))
                })
                .collect();
            used = cpu_used(&sys);
            let now: std::collections::HashSet<Pid> = sys
                .processes()
                .iter()
                .filter(|(pid, p)| {
                    let busy = cpu.get(pid).copied().unwrap_or(0.0);
                    counts_as_stuck(
                        is_thread(p),
                        p.run_time(),
                        timeout_secs,
                        looks_stuck(busy, p.status()),
                    )
                })
                .map(|(pid, _)| *pid)
                .collect();
            stuck = Some(match stuck {
                None => now,
                Some(before) => before.intersection(&now).copied().collect(),
            });
        }

        let stuck = stuck.unwrap_or_default();
        Ok(sys
            .processes()
            .iter()
            .filter(|(pid, _)| stuck.contains(pid))
            .map(|(pid, p)| {
                let mut process = Process::from_sysinfo(*pid, p);
                // the figure the last read measured, not sysinfo's own
                if let Some(&busy) = cpu.get(pid) {
                    process.cpu_percent = busy;
                }
                process
            })
            .collect())
    }

    /// Whether this process looks stuck now, measured the way `find_stuck` does
    /// (stuck in two readings a second apart), for a process named as a target.
    pub fn looks_stuck_now(&self) -> bool {
        let pid = Pid::from_u32(self.pid);
        let mut sys = System::new();
        let mut read_at = std::time::Instant::now();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
        let mut used = sys
            .process(pid)
            .filter(|p| self.is_same(p))
            .map(|p| p.accumulated_cpu_time());
        (0..STUCK_READINGS).all(|_| {
            std::thread::sleep(STUCK_READING_GAP);
            let started = std::time::Instant::now();
            sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
            let wall = started - read_at;
            read_at = started;
            let (Some(before), Some(p)) = (used, sys.process(pid).filter(|p| self.is_same(p)))
            else {
                return false;
            };
            let after = p.accumulated_cpu_time();
            used = Some(after);
            looks_stuck(cpu_percent_between(before, after, wall), p.status())
        })
    }

    /// CPU use over `window` (at least sysinfo's minimum), in percent of one core;
    /// None if the process is gone. A reading over a few milliseconds is ~0 for
    /// any process, so a real window is needed.
    pub fn cpu_over(&self, window: Duration) -> Option<f32> {
        let pid = Pid::from_u32(self.pid);
        let mut sys = System::new();
        let started = std::time::Instant::now();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
        let before = sys
            .process(pid)
            .filter(|p| self.is_same(p))
            .map(|p| p.accumulated_cpu_time())?;
        std::thread::sleep(window.max(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL));
        let wall = started.elapsed();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
        sys.process(pid)
            .filter(|p| self.is_same(p))
            .map(|p| cpu_percent_between(before, p.accumulated_cpu_time(), wall))
    }

    /// Measure the CPU use of these processes over 0.2 s and set `cpu_percent`,
    /// in percent of one core. A lookup reads CPU a few milliseconds after its
    /// first refresh, which is ~0% even for a process using a whole core, so
    /// commands call this before they show, filter or sort by CPU. One 0.2 s wait
    /// covers the whole batch; commands that never show CPU skip it.
    ///
    /// CPU is the CPU time each process used between two reads, over the wall
    /// time we measure between them. sysinfo's own `cpu_usage` divides by its
    /// system-wide tick interval instead, which read about 2% for a process
    /// spinning at 100% in some runs.
    pub fn measure_cpu(processes: &mut [Process]) {
        if processes.is_empty() {
            return;
        }
        let pids: Vec<Pid> = processes.iter().map(|p| Pid::from_u32(p.pid)).collect();
        let which = sysinfo::ProcessesToUpdate::Some(&pids);
        let cpu_only = sysinfo::ProcessRefreshKind::nothing().with_cpu();
        let mut sys = System::new();

        let first = std::time::Instant::now();
        sys.refresh_processes_specifics(which, true, cpu_only);
        let before = cpu_used(&sys);
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        // each refresh reads the processes in the same order, so timing both
        // from their start gives every process the same window
        let wall = first.elapsed();
        sys.refresh_processes_specifics(which, true, cpu_only);

        for process in processes.iter_mut() {
            let pid = Pid::from_u32(process.pid);
            let (Some(current), Some(&cpu_before)) = (sys.process(pid), before.get(&pid)) else {
                continue;
            };
            if process.is_same(current) {
                process.cpu_percent =
                    cpu_percent_between(cpu_before, current.accumulated_cpu_time(), wall);
            }
        }
    }

    /// Force kill the process (SIGKILL on Unix, taskkill /F on Windows)
    pub fn kill(&self) -> Result<()> {
        let mut sys = System::new();
        sys.refresh_processes(
            sysinfo::ProcessesToUpdate::Some(&[Pid::from_u32(self.pid)]),
            true,
        );

        match sys.process(Pid::from_u32(self.pid)) {
            Some(proc) if self.is_same(proc) => kill_outcome(proc.kill(), self.pid),
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
        self.kill_and_wait_within(Duration::from_secs(5))
    }

    /// `kill_and_wait` with the time to wait for the process to be gone
    fn kill_and_wait_within(&self, wait: Duration) -> Result<()> {
        match self.kill() {
            // it exited on its own just before the kill
            Err(ProcError::ProcessNotFound(_)) => return Ok(()),
            other => other?,
        }
        let deadline = std::time::Instant::now() + wait;
        while std::time::Instant::now() < deadline {
            if !self.is_running() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Err(ProcError::SignalError(format!(
            "Process {} is still running {} seconds after SIGKILL",
            self.pid,
            wait.as_secs()
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

    /// Graceful termination (Windows): `taskkill` without `/F` asks the
    /// process to close its windows, so it fails for a console program, which
    /// has none. That failure is returned, with taskkill's reason, rather than
    /// reported as success.
    #[cfg(windows)]
    pub fn terminate(&self) -> Result<()> {
        use std::process::Command;

        self.ensure_same_process()?;
        let output = Command::new("taskkill")
            .args(["/PID", &self.pid.to_string()])
            .output()
            .map_err(|e| ProcError::SystemError(e.to_string()))?;

        taskkill_outcome(&output)
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
        Ok(Self::orphans_among(Self::find_all()?))
    }

    /// The orphans among `all`
    fn orphans_among(all: Vec<Process>) -> Vec<Process> {
        all.into_iter()
            .filter(|p| {
                if let Some(ppid) = p.parent_pid {
                    ppid == 1 && p.pid != 1 && !Self::is_system_process(p)
                } else {
                    false
                }
            })
            .collect()
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

    /// Convert from sysinfo Process
    pub(crate) fn from_sysinfo(pid: Pid, proc: &sysinfo::Process) -> Self {
        let cmd_vec = proc.cmd();
        // Every text field is set by whoever runs the process: escape control
        // characters before anything prints them
        let command = if cmd_vec.is_empty() {
            None
        } else {
            let joined = cmd_vec
                .iter()
                .map(|s| s.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ");
            Some(escape_control(&joined).into_owned())
        };

        let exe_path = proc
            .exe()
            .map(|p| escape_control(&p.to_string_lossy()).into_owned());
        let cwd = proc.cwd().map(|p| {
            let path = strip_deleted_marker(p.to_string_lossy().to_string());
            escape_control(&path).into_owned()
        });

        Process {
            pid: pid.as_u32(),
            name: escape_control(&proc.name().to_string_lossy()).into_owned(),
            exe_path,
            cwd,
            command,
            cpu_percent: proc.cpu_usage(),
            memory_mb: proc.memory() as f64 / 1024.0 / 1024.0,
            status: ProcessStatus::from(proc.status()),
            user: proc
                .user_id()
                .map(|uid| escape_control(&user_name(uid)).into_owned()),
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
    fn cpu_is_the_cpu_time_used_over_the_wall_time() {
        let second = Duration::from_secs(1);
        // a full core for a second, half a core, and nothing
        assert_eq!(cpu_percent_between(5_000, 6_000, second), 100.0);
        assert_eq!(cpu_percent_between(5_000, 5_500, second), 50.0);
        assert_eq!(cpu_percent_between(5_000, 5_000, second), 0.0);
        // two cores' worth reads 200%, in percent of one core
        assert_eq!(cpu_percent_between(0, 2_000, second), 200.0);
        // a counter that went backwards (a reused PID) or no time passing is 0
        assert_eq!(cpu_percent_between(6_000, 5_000, second), 0.0);
        assert_eq!(cpu_percent_between(0, 1_000, Duration::ZERO), 0.0);
    }

    #[test]
    fn a_failed_command_is_explained_in_its_own_words() {
        let reason = "ERROR: The process with PID 7 could not be terminated.";
        assert_eq!(
            command_failure(b"", reason.as_bytes(), "exit code: 1"),
            reason
        );
        // stdout when stderr is empty, trimmed
        assert_eq!(
            command_failure(b"  not here\r\n", b"", "exit code: 1"),
            "not here"
        );
        // taskkill wraps its reason over several lines; the error keeps them on one
        assert_eq!(
            command_failure(
                b"",
                b"ERROR: first part\r\n  second part.\r\n\r\n",
                "exit code: 1"
            ),
            "ERROR: first part second part."
        );
        assert_eq!(
            command_failure(b"", b"  \n", "exit code: 128"),
            "exited with exit code: 128"
        );
    }

    #[test]
    fn busy_means_over_half_a_core_or_blocked_in_the_kernel() {
        assert!(looks_stuck(50.1, SysProcessStatus::Run));
        assert!(!looks_stuck(50.0, SysProcessStatus::Run));
        assert!(!looks_stuck(0.0, SysProcessStatus::Sleep));
        assert!(looks_stuck(0.0, SysProcessStatus::UninterruptibleDiskSleep));
    }

    #[test]
    fn a_reading_counts_as_stuck_from_the_timeout_on() {
        assert!(counts_as_stuck(false, 0, 0, true));
        assert!(!counts_as_stuck(false, 59, 60, true));
        assert!(counts_as_stuck(false, 60, 60, true));
        assert!(counts_as_stuck(false, 61, 60, true));
        // and only a busy process, never a thread
        assert!(!counts_as_stuck(false, 60, 60, false));
        assert!(!counts_as_stuck(true, 60, 60, true));
    }

    #[test]
    fn a_process_that_ran_the_timeout_has_run_long_enough() {
        // --timeout 0 finds a process that reads 0 seconds old
        assert!(ran_long_enough(0, 0));
        assert!(ran_long_enough(5, 5));
        assert!(ran_long_enough(6, 5));
        assert!(!ran_long_enough(4, 5));
    }

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
        let start = me.start_time.expect("a running process has a start time");
        me.start_time = Some(start - 3600);
        assert!(!me.is_running(), "a reused PID counted as the same process");
    }

    #[test]
    fn test_escape_control_makes_terminal_input_harmless() {
        // Ordinary text, including non-ASCII, is untouched
        assert_eq!(escape_control("node server.js"), "node server.js");
        assert_eq!(escape_control("café ✓ 日本"), "café ✓ 日本");
        // ESC sequences: colours, window title (OSC), clipboard (OSC 52)
        assert_eq!(escape_control("a\x1b[31mb"), "a\\x1b[31mb");
        assert_eq!(escape_control("\x1b]0;pwned\x07"), "\\x1b]0;pwned\\x07");
        // Line breaks and tabs would split or shift a table row
        assert_eq!(escape_control("one\ntwo\tthree\r"), "one\\ntwo\\tthree\\r");
        // DEL, and C1 controls such as the 8-bit CSI (U+009B)
        assert_eq!(escape_control("x\x7fy\u{9b}2J"), "x\\x7fy\\x9b2J");
        // Bidirectional overrides can reorder text on screen ("Trojan Source")
        assert_eq!(escape_control("ab\u{202e}cd"), "ab\\u{202e}cd");
    }

    #[cfg(unix)]
    #[test]
    fn test_looks_stuck_needs_sustained_cpu() {
        let spawn = |script: &str| {
            std::process::Command::new("sh")
                .args(["-c", script])
                .spawn()
                .unwrap()
        };
        let mut spinning = spawn("while :; do :; done");
        let mut idle = spawn("exec sleep 300");
        std::thread::sleep(Duration::from_millis(300));
        let record = |pid: u32| Process::find_by_pid(pid).unwrap().unwrap();

        let spinning_stuck = record(spinning.id()).looks_stuck_now();
        let idle_stuck = record(idle.id()).looks_stuck_now();
        for child in [&mut spinning, &mut idle] {
            let _ = child.kill();
            let _ = child.wait();
        }
        assert!(
            spinning_stuck,
            "a process spinning at 100% CPU wasn't stuck"
        );
        assert!(!idle_stuck, "an idle process counted as stuck");
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

        // Every system directory counts, each checked on its own
        for exe in [
            "/System/x",
            "/usr/libexec/x",
            "/usr/sbin/x",
            "/sbin/x",
            "/usr/bin/x",
            "/usr/lib/x",
            "/usr/lib64/x",
            "/lib/x",
            "/lib64/x",
            "/opt/x",
            "/snap/x",
        ] {
            assert!(
                Process::is_system_process(&make_proc(Some(exe), Some("/"))),
                "{exe}"
            );
        }
        // a daemon with cwd "/" outside them still counts: nothing says it is a user's
        assert!(Process::is_system_process(&make_proc(
            Some("/srv/x"),
            Some("/")
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

    /// A record of a process that doesn't exist
    fn ghost() -> Process {
        Process {
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
        }
    }

    #[test]
    fn every_sysinfo_status_maps_to_ours() {
        for (sys, ours) in [
            (SysProcessStatus::Run, ProcessStatus::Running),
            (SysProcessStatus::Sleep, ProcessStatus::Sleeping),
            (SysProcessStatus::Stop, ProcessStatus::Stopped),
            (SysProcessStatus::Zombie, ProcessStatus::Zombie),
            (SysProcessStatus::Dead, ProcessStatus::Dead),
            (SysProcessStatus::Idle, ProcessStatus::Unknown),
        ] {
            assert_eq!(ProcessStatus::from(sys), ours, "{sys:?}");
        }
    }

    #[test]
    fn a_process_that_is_gone_does_not_look_stuck() {
        assert!(!ghost().looks_stuck_now());
    }

    #[test]
    fn killing_a_process_that_is_already_gone_is_success() {
        assert!(matches!(ghost().kill(), Err(ProcError::ProcessNotFound(_))));
        ghost().kill_and_wait().unwrap();
    }

    #[test]
    fn a_kill_that_was_not_delivered_is_an_error() {
        kill_outcome(true, 7).unwrap();
        let err = kill_outcome(false, 7).unwrap_err();
        assert!(
            matches!(&err, ProcError::SignalError(m) if m == "Failed to kill process 7"),
            "{err:?}"
        );
    }

    #[test]
    fn taskkill_without_force_reports_what_it_said() {
        #[cfg(unix)]
        use std::os::unix::process::ExitStatusExt;
        #[cfg(windows)]
        use std::os::windows::process::ExitStatusExt;
        use std::process::{ExitStatus, Output};

        let output = |raw, stdout: &str, stderr: &str| Output {
            status: ExitStatus::from_raw(raw),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        };
        taskkill_outcome(&output(0, "SUCCESS: sent termination signal", "")).unwrap();

        // exit code 1 on Windows; the same wait status on Unix
        #[cfg(unix)]
        let failed = 1 << 8;
        #[cfg(windows)]
        let failed = 1;
        let err =
            taskkill_outcome(&output(failed, "", "ERROR: could not be terminated")).unwrap_err();
        assert!(
            matches!(&err, ProcError::SignalError(m) if m == "ERROR: could not be terminated"),
            "{err:?}"
        );
    }

    #[test]
    fn orphans_are_the_children_of_init_that_are_not_system_daemons() {
        let proc = |pid: u32, ppid: Option<u32>, exe: Option<&str>, cwd: Option<&str>| Process {
            pid,
            name: format!("p{pid}"),
            exe_path: exe.map(String::from),
            cwd: cwd.map(String::from),
            parent_pid: ppid,
            ..ghost()
        };
        let orphans = Process::orphans_among(vec![
            // a user's process whose parent died
            proc(10, Some(1), Some("/home/z/app"), Some("/home/z")),
            // a system daemon: cwd "/" and an exe in a system directory
            proc(11, Some(1), Some("/usr/sbin/sshd"), Some("/")),
            // init itself, and a process with a live parent
            proc(1, Some(1), Some("/sbin/init"), Some("/home/z")),
            proc(12, Some(500), Some("/home/z/app"), Some("/home/z")),
            // no parent known
            proc(13, None, Some("/home/z/app"), Some("/home/z")),
        ]);
        let pids: Vec<u32> = orphans.iter().map(|p| p.pid).collect();
        assert_eq!(pids, vec![10]);
    }

    #[cfg(unix)]
    #[test]
    fn a_missing_lsof_finds_no_open_file_owners() {
        let found = Process::find_by_open_file_with("proc-no-such-lsof", std::path::Path::new("/"));
        assert!(found.unwrap().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_process_still_alive_when_the_wait_ends_is_an_error() {
        let mut child = std::process::Command::new("sleep")
            .arg("300")
            .spawn()
            .unwrap();
        let record = Process::find_by_pid(child.id()).unwrap().unwrap();
        // no time to wait: the SIGKILL is sent, but the process isn't seen gone
        let result = record.kill_and_wait_within(Duration::ZERO);
        let _ = child.kill();
        let _ = child.wait();
        assert!(
            matches!(&result, Err(ProcError::SignalError(m))
                if m == &format!("Process {} is still running 0 seconds after SIGKILL", record.pid)),
            "{result:?}"
        );
    }

    /// One kinfo_proc record, with the fields `zombies_in` reads
    #[cfg(target_os = "macos")]
    fn kinfo(pid: u32, stat: u8, comm: &str, ruid: u32, ppid: u32, start: u64) -> Vec<u8> {
        let mut rec = vec![0u8; KINFO_PROC_SIZE];
        rec[0..8].copy_from_slice(&start.to_ne_bytes());
        rec[36] = stat;
        rec[40..44].copy_from_slice(&pid.to_ne_bytes());
        rec[243..243 + comm.len()].copy_from_slice(comm.as_bytes());
        rec[392..396].copy_from_slice(&ruid.to_ne_bytes());
        rec[560..564].copy_from_slice(&ppid.to_ne_bytes());
        rec
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn zombies_are_read_from_the_kernels_table() {
        let known = std::collections::HashSet::from([30]);
        let table: Vec<u8> = [
            kinfo(10, 5, "defunct-app", 0, 99, 1_700_000_000),
            // no name left, and no start time
            kinfo(11, 5, "", 0, 1, 0),
            // alive, not a zombie
            kinfo(20, 2, "alive", 0, 1, 1_700_000_000),
            // already listed by sysinfo
            kinfo(30, 5, "known", 0, 1, 1_700_000_000),
            // the kernel's own slot
            kinfo(0, 5, "kernel", 0, 0, 0),
        ]
        .concat();

        let zombies = zombies_in(&table, &known);
        let pids: Vec<u32> = zombies.iter().map(|z| z.pid).collect();
        assert_eq!(pids, vec![10, 11]);

        let named = &zombies[0];
        assert_eq!(named.name, "defunct-app");
        assert_eq!(named.status, ProcessStatus::Zombie);
        assert_eq!(named.parent_pid, Some(99));
        assert_eq!(named.start_time, Some(1_700_000_000));
        assert!(named.uptime_secs.unwrap() > 0);
        assert_eq!(named.user.as_deref(), Some("root"));

        let unnamed = &zombies[1];
        assert_eq!(unnamed.name, "<defunct>");
        assert_eq!(unnamed.start_time, None);
        assert_eq!(unnamed.uptime_secs, None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_table_of_an_unknown_layout_shows_no_zombies() {
        let known = std::collections::HashSet::new();
        let mut table = kinfo(10, 5, "z", 0, 1, 0);
        table.push(0);
        assert!(zombies_in(&table, &known).is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_refused_sysctl_gives_an_empty_table() {
        use std::cell::Cell;

        // refused when asked for the size
        assert!(read_proc_table(|_, _| -1).is_empty());

        // refused when asked for the table
        let calls = Cell::new(0);
        let table = read_proc_table(|_, size| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                *size = 2 * KINFO_PROC_SIZE;
                0
            } else {
                -1
            }
        });
        assert_eq!(calls.get(), 2);
        assert!(table.is_empty());

        // answered: the table is cut to the size the kernel reports
        let calls = Cell::new(0);
        let table = read_proc_table(|buf, size| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                *size = 2 * KINFO_PROC_SIZE;
            } else {
                assert_eq!(*size, 66 * KINFO_PROC_SIZE, "room for 64 newcomers");
                // SAFETY: `buf` has `*size` writable bytes
                unsafe { buf.cast::<u8>().write(7) };
                *size = KINFO_PROC_SIZE;
            }
            0
        });
        assert_eq!(table.len(), KINFO_PROC_SIZE);
        assert_eq!(table[0], 7);
    }
}
