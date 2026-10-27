//! The commands that act on processes: `wait`, `stop`, `kill`, `freeze`,
//! `thaw`, `unstick` and `orphans --kill`, driven through the real binary.
//!
//! Every process these tests aim proc at is one the test started: a copy of the
//! listener example (`examples/listener.rs`) under a name and path of its own in
//! a fresh temp directory, a shell the test owns, or an orphan the test made.
//! They are named by PID or scoped with `--in <that directory>`, and killed and
//! reaped on drop. Timing assertions live in tests/timing.rs; the limits here
//! are only there to stop a broken command from hanging the test.

use assert_cmd::Command;
use proc_cli::core::filters::strip_verbatim_prefix;
use serde_json::Value;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// A busy process's CPU reads well above this once measured, and ~0 when it
/// isn't (a lookup reads CPU a few milliseconds after its first refresh). Not
/// 50: a full core read 49% on a loaded, instrumented CI runner
const MEASURED: f64 = 20.0;

fn proc_cmd() -> Command {
    let mut cmd = Command::cargo_bin("proc").unwrap();
    cmd.env("NO_COLOR", "1");
    cmd
}

/// What one run of proc printed, and its exit code
struct Run {
    stdout: String,
    stderr: String,
    code: i32,
}

fn run(args: &[&str]) -> Run {
    let output = proc_cmd().args(args).output().unwrap();
    Run {
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
        code: output.status.code().unwrap(),
    }
}

/// Run proc and parse stdout as exactly one JSON document, with the exit code.
fn run_json(args: &[&str]) -> (Value, i32) {
    let r = run(args);
    let json: Value = serde_json::from_str(&r.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not a single JSON document ({e}):\n{}\nstderr:\n{}",
            r.stdout, r.stderr
        )
    });
    (json, r.code)
}

/// `target/debug/examples/listener`, found relative to this test binary
/// (`target/debug/deps/cmd_actions-<hash>`)
fn listener_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("examples");
    path.push(format!("listener{}", std::env::consts::EXE_SUFFIX));
    assert!(path.exists(), "build the example first: {}", path.display());
    path
}

/// A fresh temp directory, as processes report paths: symlinks resolved and no
/// Windows `\\?\` prefix
fn fresh_dir() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().canonicalize().unwrap();
    let dir = PathBuf::from(strip_verbatim_prefix(&dir.to_string_lossy()));
    (tmp, dir)
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Run the copy, with its cwd in `dir`. On Linux a file another test thread has
/// just written can be "busy" (ETXTBSY) for an instant: a fork in a parallel
/// test still holds the write handle until it execs. That clears in
/// milliseconds, so try again, a bounded number of times.
fn spawn_copy(exe: &Path, args: &[&str], dir: &Path) -> Child {
    let mut attempts = 0;
    loop {
        let mut command = std::process::Command::new(exe);
        command
            .args(args)
            .current_dir(dir)
            .stdout(Stdio::piped())
            .stdin(Stdio::null());
        default_sigint(&mut command);
        let spawned = command.spawn();
        match spawned {
            Ok(child) => return child,
            Err(e) if attempts < 50 && is_text_file_busy(&e) => {
                attempts += 1;
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => panic!("spawn the listener copy: {e:?}"),
        }
    }
}

/// A process started from a background job (as cargo may run this test) inherits
/// "ignore SIGINT"; unstick's step 2 needs the default, which ends the listener
#[cfg(unix)]
fn default_sigint(command: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY: only signal(2), which is async-signal-safe, runs between fork and exec
    unsafe {
        command.pre_exec(|| {
            libc::signal(libc::SIGINT, libc::SIG_DFL);
            Ok(())
        });
    }
}

#[cfg(not(unix))]
fn default_sigint(_: &mut std::process::Command) {}

#[cfg(unix)]
fn is_text_file_busy(e: &std::io::Error) -> bool {
    e.raw_os_error() == Some(libc::ETXTBSY)
}

#[cfg(not(unix))]
fn is_text_file_busy(_: &std::io::Error) -> bool {
    false
}

/// Held for as long as a busy process runs, so no more than one spins at a time
static BUSY: Mutex<()> = Mutex::new(());

fn busy_lock() -> MutexGuard<'static, ()> {
    BUSY.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A process the test owns: a copy of the listener example, run with its cwd in
/// a fresh temp directory. Killed and reaped on drop, whatever a test did to it.
struct Owned {
    child: Child,
    /// A name no other process has, short enough for any OS to report whole
    name: String,
    /// Keeps the directory alive; `dir` is its path as processes report it
    _tmp: tempfile::TempDir,
    dir: PathBuf,
    /// Released after the child is killed (fields drop after `Drop::drop`)
    _busy: Option<MutexGuard<'static, ()>>,
}

impl Owned {
    /// A listener that sleeps
    fn listener() -> Self {
        Self::start(&[], None)
    }

    /// A listener that sleeps and ignores SIGTERM (on Unix)
    fn ignoring_term() -> Self {
        Self::start(&["--ignore-term"], None)
    }

    /// A listener that spins on a core: stuck by proc's measure (over half a
    /// core in two readings a second apart). Returns once proc sees it as stuck.
    fn busy() -> Self {
        let guard = busy_lock();
        let mut busy = Self::start(&["--busy"], None);
        busy._busy = Some(guard);
        // proc has no CPU reading for a process that has used no CPU yet, so a
        // process started this instant is not found until a later look
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let (json, _) = run_json(&["stuck", "--timeout", "0", "--in", &busy.dir(), "--json"]);
            if lists(&json, "processes", busy.pid()) {
                return busy;
            }
            if Instant::now() >= deadline {
                panic!("never seen as stuck: {json}\n{}", busy.what_proc_saw());
            }
        }
    }

    fn start(args: &[&str], exe: Option<&Path>) -> Self {
        let (tmp, dir) = fresh_dir();
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let name = format!("pca{}n{n}x", std::process::id());
        let exe = match exe {
            Some(shared) => shared.to_path_buf(),
            None => {
                let copy = dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
                std::fs::copy(listener_path(), &copy).expect("copy the listener example");
                copy
            }
        };
        let mut child = spawn_copy(&exe, args, &dir);
        let mut line = String::new();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let read = stdout.read_line(&mut line);
        let port: Option<u16> = read
            .ok()
            .and_then(|_| line.trim().strip_prefix("PORT "))
            .and_then(|p| p.parse().ok());
        match port {
            Some(_) => Owned {
                child,
                name,
                _tmp: tmp,
                dir,
                _busy: None,
            },
            None => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("the listener printed {line:?} instead of `PORT <n>`");
            }
        }
    }

    /// What proc and the OS say about this process, for a failure message
    fn what_proc_saw(&mut self) -> String {
        let dir = self.dir();
        let pid = self.pid().to_string();
        let text = |args: &[&str]| {
            let out = proc_cmd().args(args).output().unwrap();
            format!(
                "exit {:?}\n{}{}",
                out.status.code(),
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            )
        };
        format!(
            "proc list --in {dir} --json:\n{}\nproc info {pid} --json:\n{}\nthe child's try_wait: {:?}",
            text(&["list", "--in", &dir, "--json"]),
            text(&["info", &pid, "--json"]),
            self.child.try_wait()
        )
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn pid_arg(&self) -> String {
        self.pid().to_string()
    }

    fn dir(&self) -> String {
        self.dir.to_string_lossy().into_owned()
    }

    /// Whether the child is still there: not exited (it is ours, so this also reaps it)
    fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Wait for the child to end
    fn ended_within(&mut self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        while Instant::now() < deadline {
            if !self.alive() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    /// End the child now, as the test's way of making a process exit
    fn exit_now(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// The signal that ended the child (None if it exited by itself or is still there)
    #[cfg(unix)]
    fn ended_by_signal(&mut self, limit: Duration) -> Option<i32> {
        use std::os::unix::process::ExitStatusExt;
        let deadline = Instant::now() + limit;
        while Instant::now() < deadline {
            if let Ok(Some(status)) = self.child.try_wait() {
                return status.signal();
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        None
    }

    /// A PID that was this process's, with nothing behind it now
    fn gone_pid(mut self) -> String {
        let pid = self.pid_arg();
        self.exit_now();
        pid
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Whether the JSON list `key` has a process with this PID
fn lists(json: &Value, key: &str, pid: u32) -> bool {
    json[key].as_array().is_some_and(|items| {
        items
            .iter()
            .any(|p| p["pid"].as_u64() == Some(u64::from(pid)))
    })
}

/// A running `proc` whose output the test reads line by line while it waits,
/// so the test can act (end a process) at a known point. Killed and reaped on drop.
struct Live {
    child: Child,
    lines: Receiver<String>,
    seen: Vec<String>,
}

/// A `Live` run's output and exit code
struct Finished {
    lines: Vec<String>,
    stderr: String,
    code: i32,
}

impl Live {
    fn start(args: &[&str]) -> Self {
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_proc"))
            .args(args)
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn proc");
        let out = child.stdout.take().unwrap();
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Live {
            child,
            lines,
            seen: Vec::new(),
        }
    }

    /// Read lines until one contains `needle`; returns it
    fn line_with(&mut self, needle: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(line) => {
                    self.seen.push(line.clone());
                    if line.contains(needle) {
                        return line;
                    }
                }
                Err(_) => panic!(
                    "never saw {needle:?}; proc printed:\n{}",
                    self.seen.join("\n")
                ),
            }
        }
    }

    /// Wait for proc to end; everything it printed, and its exit code
    fn finish(&mut self) -> Finished {
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "proc did not exit; it printed:\n{}",
                self.seen.join("\n")
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        while let Ok(line) = self.lines.recv_timeout(Duration::from_secs(5)) {
            self.seen.push(line);
        }
        let mut stderr = String::new();
        if let Some(mut err) = self.child.stderr.take() {
            let _ = err.read_to_string(&mut stderr);
        }
        Finished {
            lines: self.seen.clone(),
            stderr,
            code: status.code().unwrap(),
        }
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ---- wait ----

#[test]
fn wait_reports_the_exit_of_a_process() {
    let mut owned = Owned::listener();
    let pid = owned.pid_arg();
    let mut waiter = Live::start(&["wait", &pid, "--timeout", "60"]);
    waiter.line_with("Waiting for 1 process to exit");
    owned.exit_now();
    waiter.line_with(&format!("[PID {pid}] exited after"));
    waiter.line_with("All 1 process exited after");
    let done = waiter.finish();
    assert_eq!(done.code, 0, "{}", done.stderr);
}

#[test]
fn wait_prints_a_status_line_each_interval_and_counts_the_exits() {
    let mut first = Owned::listener();
    let mut second = Owned::listener();
    let targets = format!("{},{}", first.pid(), second.pid());
    let mut waiter = Live::start(&["wait", &targets, "--interval", "1", "--timeout", "60"]);
    waiter.line_with("Waiting for 2 processes to exit");
    let line = waiter.line_with("2 still running");
    assert!(
        line.contains(&format!("[{}]", first.pid()))
            && line.contains(&format!("[{}]", second.pid())),
        "{line}"
    );
    first.exit_now();
    waiter.line_with(&format!("[PID {}] exited after", first.pid()));
    let line = waiter.line_with("1 still running");
    assert!(line.contains(&format!("[{}]", second.pid())), "{line}");
    assert!(line.contains("(1 exited)"), "{line}");
    second.exit_now();
    waiter.line_with("All 2 processes exited after");
    let done = waiter.finish();
    assert_eq!(done.code, 0, "{}", done.stderr);
}

#[test]
fn wait_quiet_prints_no_status_lines() {
    let mut owned = Owned::listener();
    let pid = owned.pid_arg();
    let mut waiter = Live::start(&[
        "wait",
        &pid,
        "--interval",
        "1",
        "--quiet",
        "--timeout",
        "60",
    ]);
    waiter.line_with("Waiting for 1 process to exit");
    // longer than two intervals: a status line would have come by now
    std::thread::sleep(Duration::from_millis(2500));
    owned.exit_now();
    let done = waiter.finish();
    assert_eq!(done.code, 0, "{}", done.stderr);
    assert!(
        done.lines.iter().all(|l| !l.contains("still running")),
        "{:?}",
        done.lines
    );
    assert!(
        done.lines
            .iter()
            .any(|l| l.contains("All 1 process exited after")),
        "{:?}",
        done.lines
    );
}

#[test]
fn wait_verbose_lists_each_process_with_its_cpu() {
    let mut owned = Owned::listener();
    let pid = owned.pid_arg();
    let r = run(&["wait", &pid, "--verbose", "--timeout", "1"]);
    let line = r
        .stdout
        .lines()
        .find(|l| l.contains(&format!("[PID {pid}]")))
        .unwrap_or_else(|| panic!("no verbose line for {pid}:\n{}", r.stdout));
    assert!(line.contains("% CPU,"), "{line}");
    assert!(line.contains(&owned.name), "{line}");
    assert!(owned.alive());
}

#[test]
fn wait_that_times_out_names_what_is_still_running() {
    let owned = Owned::listener();
    let pid = owned.pid_arg();
    let r = run(&["wait", &pid, "--timeout", "1"]);
    assert_eq!(r.code, 1, "{}{}", r.stdout, r.stderr);
    assert!(r.stderr.contains("timed out"), "{}", r.stderr);
    assert!(r.stderr.contains("1 still running"), "{}", r.stderr);
    assert!(r.stderr.contains(&format!("[{pid}]")), "{}", r.stderr);
}

#[test]
fn wait_json_timeout_lists_the_process_still_running() {
    let owned = Owned::listener();
    let pid = owned.pid_arg();
    let (json, code) = run_json(&["wait", &pid, "--timeout", "1", "--interval", "1", "--json"]);
    assert_eq!(code, 1, "{json}");
    assert_eq!(json["action"], "wait");
    assert_eq!(json["success"], false);
    assert_eq!(json["timed_out"], true);
    assert_eq!(json["target"], pid.as_str());
    assert_eq!(json["initial_count"], 1);
    assert_eq!(json["exited"].as_array().unwrap().len(), 0, "{json}");
    assert!(lists(&json, "still_running", owned.pid()), "{json}");
}

#[test]
fn wait_with_a_target_that_is_gone_is_not_found() {
    // wait only reads, so a reused PID could not be harmed
    let pid = Owned::listener().gone_pid();
    let (json, code) = run_json(&["wait", &pid, "--timeout", "60", "--json"]);
    assert_eq!(code, 2, "{json}");
    assert_eq!(json["success"], false);
    let r = run(&["wait", &pid, "--timeout", "60"]);
    assert_eq!(r.code, 2, "{}{}", r.stdout, r.stderr);
}

#[test]
fn wait_warns_about_a_missing_target_and_waits_for_the_rest() {
    let gone = Owned::listener().gone_pid();
    let alive = Owned::listener();
    let targets = format!("{gone},{}", alive.pid());
    let r = run(&["wait", &targets, "--timeout", "1"]);
    assert!(
        r.stdout.contains(&format!("Not found: {gone}")),
        "{}",
        r.stdout
    );
    assert!(
        r.stdout.contains("Waiting for 1 process to exit"),
        "{}",
        r.stdout
    );
    assert_eq!(r.code, 1, "{}{}", r.stdout, r.stderr);
    let r = run(&["wait", &targets, "--timeout", "1", "--json"]);
    let json: Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(r.code, 1, "{json}");
    assert_eq!(json["initial_count"], 1, "{json}");
    assert!(lists(&json, "still_running", alive.pid()), "{json}");
}

#[test]
fn wait_with_a_filter_that_matches_nothing_is_not_found() {
    let owned = Owned::listener();
    let pid = owned.pid_arg();
    let r = run(&[
        "wait",
        &pid,
        "--by",
        "no-process-has-this-name",
        "--timeout",
        "1",
    ]);
    assert_eq!(r.code, 2, "{}{}", r.stdout, r.stderr);
}

/// Children of this test that exit by themselves and are not reaped until drop,
/// so once they exit they are zombies: gone, but with their PID still taken
#[cfg(unix)]
struct Short(Child);

#[cfg(unix)]
impl Short {
    fn sleeping(secs: &str) -> Self {
        Short(
            std::process::Command::new("sleep")
                .arg(secs)
                .spawn()
                .unwrap(),
        )
    }
}

#[cfg(unix)]
impl Drop for Short {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(unix)]
#[test]
fn wait_json_counts_zombies_as_exited_and_reports_each() {
    let first = Short::sleeping("1");
    let second = Short::sleeping("2");
    let targets = format!("{},{}", first.0.id(), second.0.id());
    let (json, code) = run_json(&["wait", &targets, "--timeout", "60", "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["success"], true, "{json}");
    assert_eq!(json["timed_out"], false, "{json}");
    assert_eq!(json["initial_count"], 2, "{json}");
    assert!(json["still_running"].as_array().unwrap().is_empty());
    assert!(lists(&json, "exited", first.0.id()), "{json}");
    assert!(lists(&json, "exited", second.0.id()), "{json}");
    assert!(json["exited"][0]["exited_after_seconds"].is_u64(), "{json}");
    assert!(json["elapsed_human"].is_string(), "{json}");
}

#[cfg(unix)]
#[test]
fn wait_json_timeout_reports_the_exited_and_the_still_running() {
    let short = Short::sleeping("1");
    let long = Owned::listener();
    let targets = format!("{},{}", short.0.id(), long.pid());
    let (json, code) = run_json(&["wait", &targets, "--timeout", "4", "--json"]);
    assert_eq!(code, 1, "{json}");
    assert_eq!(json["timed_out"], true, "{json}");
    assert!(lists(&json, "exited", short.0.id()), "{json}");
    assert!(lists(&json, "still_running", long.pid()), "{json}");
    assert_eq!(json["initial_count"], 2, "{json}");
}

// ---- stop ----

#[test]
fn stop_dry_run_says_what_it_would_stop_and_stops_nothing() {
    let mut owned = Owned::listener();
    let pid = owned.pid_arg();
    let r = run(&["stop", &pid, "--dry-run"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(
        r.stdout.contains(&owned.name) || r.stdout.contains(&pid),
        "{}",
        r.stdout
    );
    assert!(
        r.stdout.contains("Dry run: would stop 1 process")
            || r.stderr.contains("Dry run: would stop 1 process"),
        "{}{}",
        r.stdout,
        r.stderr
    );
    let (json, code) = run_json(&["stop", &pid, "--dry-run", "--json"]);
    assert_eq!(code, 0);
    assert_eq!(json["action"], "stop");
    assert_eq!(json["dry_run"], true);
    assert_eq!(json["count"], 1);
    assert!(lists(&json, "processes", owned.pid()), "{json}");
    assert!(owned.alive());
}

#[test]
fn stop_without_yes_and_json_asks_for_yes_with_measured_cpu() {
    let mut busy = Owned::busy();
    let (json, code) = run_json(&["stop", &busy.pid_arg(), "--json"]);
    assert_eq!(code, 4, "{json}");
    assert_eq!(json["needs_confirmation"], true, "{json}");
    assert_eq!(json["processes"][0]["pid"], busy.pid(), "{json}");
    let cpu = json["processes"][0]["cpu_percent"].as_f64().unwrap();
    assert!(cpu > MEASURED, "cpu_percent {cpu} was not measured: {json}");
    assert!(busy.alive());
}

#[test]
fn stop_without_yes_and_without_a_terminal_asks_for_yes() {
    let mut owned = Owned::listener();
    let r = run(&["stop", &owned.pid_arg()]);
    assert_eq!(r.code, 4, "{}{}", r.stdout, r.stderr);
    assert!(r.stderr.contains("--yes"), "{}", r.stderr);
    assert!(owned.alive());
}

#[test]
fn stop_ends_a_process_and_says_so() {
    let mut owned = Owned::listener();
    let pid = owned.pid_arg();
    let r = run(&["stop", &pid, "--yes", "--timeout", "1"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains("Stopped 1 process"), "{}", r.stdout);
    assert!(r.stdout.contains(&format!("[PID {pid}]")), "{}", r.stdout);
    assert!(owned.ended_within(Duration::from_secs(15)));
}

#[test]
fn stop_force_kills_a_process_that_outlives_the_timeout() {
    let mut owned = Owned::ignoring_term();
    let (json, code) = run_json(&[
        "stop",
        &owned.pid_arg(),
        "--yes",
        "--timeout",
        "1",
        "--json",
    ]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["action"], "stop");
    assert_eq!(json["success"], true, "{json}");
    assert_eq!(json["succeeded_count"], 1, "{json}");
    assert!(owned.ended_within(Duration::from_secs(15)));
    // SIGTERM was ignored, so only the kill after the timeout can have ended it
    #[cfg(unix)]
    assert_eq!(
        owned.ended_by_signal(Duration::from_secs(5)),
        Some(libc::SIGKILL)
    );
}

#[test]
fn stop_handles_several_targets_at_once() {
    let mut first = Owned::listener();
    let mut second = Owned::listener();
    let targets = format!("{},{}", first.pid(), second.pid());
    let (json, code) = run_json(&["stop", &targets, "--yes", "--timeout", "1", "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["succeeded_count"], 2, "{json}");
    assert_eq!(json["failed_count"], 0, "{json}");
    assert!(first.ended_within(Duration::from_secs(15)));
    assert!(second.ended_within(Duration::from_secs(15)));
}

#[test]
fn stop_that_finds_nothing_is_not_found_and_a_missing_target_is_warned_about() {
    // `--in` a directory of the test's own: if the OS handed the gone PID to a
    // stranger, its cwd is not here, so it is still "not found" and untouched
    let gone = Owned::listener().gone_pid();
    let (_tmp, own_dir) = fresh_dir();
    let own_dir = own_dir.to_string_lossy().into_owned();
    let r = run(&["stop", &gone, "--yes", "--in", &own_dir]);
    assert_eq!(r.code, 2, "{}{}", r.stdout, r.stderr);

    let mut alive = Owned::listener();
    let targets = format!("{gone},{}", alive.pid());
    let r = run(&[
        "stop",
        &targets,
        "--yes",
        "--timeout",
        "1",
        "--in",
        &alive.dir(),
    ]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(
        r.stdout.contains(&format!("Not found: {gone}")),
        "{}",
        r.stdout
    );
    assert!(alive.ended_within(Duration::from_secs(15)));
}

#[cfg(unix)]
#[test]
fn stop_signal_sends_that_signal_instead_of_sigterm() {
    // the process ignores SIGTERM, so a plain stop would wait out the timeout
    // and kill it; HUP ends it at once, and the signal that ended it says which
    let mut owned = Owned::ignoring_term();
    let (json, code) = run_json(&[
        "stop",
        &owned.pid_arg(),
        "--signal",
        "HUP",
        "--yes",
        "--timeout",
        "20",
        "--json",
    ]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["succeeded_count"], 1, "{json}");
    assert_eq!(
        owned.ended_by_signal(Duration::from_secs(5)),
        Some(libc::SIGHUP)
    );
}

// ---- kill ----

#[test]
fn kill_ends_a_process_and_says_so() {
    let mut owned = Owned::listener();
    let pid = owned.pid_arg();
    let r = run(&["kill", &pid, "--yes"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains("Killed 1 process"), "{}", r.stdout);
    assert!(owned.ended_within(Duration::from_secs(15)));
}

#[test]
fn kill_graceful_sends_a_normal_stop_request() {
    let mut owned = Owned::listener();
    let (json, code) = run_json(&["kill", &owned.pid_arg(), "--graceful", "--yes", "--json"]);
    // on Unix a graceful kill is SIGTERM, which the listener does not ignore
    #[cfg(unix)]
    {
        assert_eq!(code, 0, "{json}");
        assert_eq!(json["succeeded_count"], 1, "{json}");
        assert_eq!(
            owned.ended_by_signal(Duration::from_secs(15)),
            Some(libc::SIGTERM)
        );
    }
    // on Windows a graceful kill is taskkill without /F, which a console
    // program (no window to close) refuses: proc says so, with taskkill's
    // reason, and the listener keeps running
    #[cfg(not(unix))]
    {
        assert_eq!(code, 1, "{json}");
        assert_eq!(json["failed_count"], 1, "{json}");
        assert!(!owned.ended_within(Duration::from_secs(2)), "{json}");
    }
}

// ---- freeze and thaw ----

/// Whether `ps` shows the process stopped (state `T`)
#[cfg(unix)]
fn stopped(pid: u32) -> bool {
    let out = std::process::Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .expect("run ps");
    String::from_utf8_lossy(&out.stdout).trim().starts_with('T')
}

#[cfg(unix)]
fn becomes_stopped(pid: u32, want: bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if stopped(pid) == want {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[cfg(unix)]
#[test]
fn freeze_stops_a_process_and_thaw_resumes_it() {
    let owned = Owned::listener();
    let pid = owned.pid_arg();
    let (json, code) = run_json(&["freeze", &pid, "--yes", "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["action"], "freeze");
    assert_eq!(json["succeeded_count"], 1, "{json}");
    assert!(
        becomes_stopped(owned.pid(), true),
        "not stopped after freeze"
    );

    let r = run(&["thaw", &pid, "--yes"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains("Thawed 1 process"), "{}", r.stdout);
    assert!(
        becomes_stopped(owned.pid(), false),
        "still stopped after thaw"
    );

    let r = run(&["freeze", &pid, "--yes"]);
    assert!(r.stdout.contains("Frozen 1 process"), "{}", r.stdout);
    let (json, code) = run_json(&["thaw", &pid, "--yes", "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["action"], "thaw");
    assert!(becomes_stopped(owned.pid(), false));
}

#[cfg(unix)]
#[test]
fn freeze_and_thaw_dry_runs_change_nothing() {
    let owned = Owned::listener();
    let pid = owned.pid_arg();
    for action in ["freeze", "thaw"] {
        let (json, code) = run_json(&[action, &pid, "--dry-run", "--json"]);
        assert_eq!(code, 0, "{json}");
        assert_eq!(json["action"], action);
        assert_eq!(json["dry_run"], true);
        assert_eq!(json["count"], 1);
        assert!(lists(&json, "processes", owned.pid()), "{json}");

        let r = run(&[action, &pid, "--dry-run"]);
        assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
        let says = format!("Dry run: would {action} 1 process");
        assert!(
            r.stdout.contains(&says) || r.stderr.contains(&says),
            "{}{}",
            r.stdout,
            r.stderr
        );
    }
    assert!(!stopped(owned.pid()));
}

#[cfg(unix)]
#[test]
fn freeze_and_thaw_without_yes_and_json_ask_for_yes_with_measured_cpu() {
    let busy = Owned::busy();
    for action in ["freeze", "thaw"] {
        let (json, code) = run_json(&[action, &busy.pid_arg(), "--json"]);
        assert_eq!(code, 4, "{action}: {json}");
        assert_eq!(json["needs_confirmation"], true, "{action}: {json}");
        assert_eq!(json["processes"][0]["pid"], busy.pid(), "{action}: {json}");
        let cpu = json["processes"][0]["cpu_percent"].as_f64().unwrap();
        assert!(
            cpu > MEASURED,
            "{action}: cpu_percent {cpu} was not measured: {json}"
        );
    }
    assert!(!stopped(busy.pid()));
}

#[cfg(unix)]
#[test]
fn freeze_and_thaw_find_nothing_for_a_gone_target_and_warn_about_it() {
    let gone = Owned::listener().gone_pid();
    let alive = Owned::listener();
    // `--in` the test's own directories: a stranger that got the gone PID is
    // filtered out, so nothing but our own process can be frozen or thawed
    let (_tmp, own_dir) = fresh_dir();
    let own_dir = own_dir.to_string_lossy().into_owned();
    for action in ["freeze", "thaw"] {
        let r = run(&[action, &gone, "--yes", "--in", &own_dir]);
        assert_eq!(r.code, 2, "{action}: {}{}", r.stdout, r.stderr);

        let targets = format!("{gone},{}", alive.pid());
        let r = run(&[action, &targets, "--yes", "--in", &alive.dir()]);
        assert_eq!(r.code, 0, "{action}: {}{}", r.stdout, r.stderr);
        assert!(
            r.stdout.contains(&format!("Not found: {gone}")),
            "{action}: {}",
            r.stdout
        );
    }
    assert!(!stopped(alive.pid()));
}

#[cfg(unix)]
#[test]
fn freeze_and_thaw_handle_several_targets_and_respect_filters() {
    let first = Owned::listener();
    let second = Owned::listener();
    let targets = format!("{},{}", first.pid(), second.pid());
    let (json, code) = run_json(&["freeze", &targets, "--yes", "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["succeeded_count"], 2, "{json}");
    assert!(becomes_stopped(first.pid(), true));
    assert!(becomes_stopped(second.pid(), true));

    // a directory that holds neither process filters both out
    let (_other, other_dir) = fresh_dir();
    let r = run(&[
        "thaw",
        &targets,
        "--yes",
        "--in",
        &other_dir.to_string_lossy(),
    ]);
    assert_eq!(r.code, 2, "{}{}", r.stdout, r.stderr);
    assert!(stopped(first.pid()) && stopped(second.pid()));

    let (json, code) = run_json(&["thaw", &targets, "--yes", "--in", &first.dir(), "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["succeeded_count"], 1, "{json}");
    assert!(becomes_stopped(first.pid(), false));
    assert!(stopped(second.pid()));
    let r = run(&[
        "thaw",
        &second.pid_arg(),
        "--yes",
        "--by",
        "no-process-has-this-name",
    ]);
    assert_eq!(r.code, 2, "{}{}", r.stdout, r.stderr);
    let (json, _) = run_json(&["thaw", &second.pid_arg(), "--yes", "--json"]);
    assert_eq!(json["succeeded_count"], 1, "{json}");
}

#[cfg(not(unix))]
#[test]
fn freeze_and_thaw_are_not_supported_on_windows() {
    let owned = Owned::listener();
    for action in ["freeze", "thaw"] {
        let r = run(&[action, &owned.pid_arg(), "--yes"]);
        assert_eq!(r.code, 4, "{action}: {}{}", r.stdout, r.stderr);
        assert!(
            r.stderr.contains("not supported on Windows"),
            "{action}: {}",
            r.stderr
        );
    }
}

// ---- orphans ----

#[test]
fn orphans_with_none_in_the_directory_says_so() {
    let (_tmp, dir) = fresh_dir();
    let dir = dir.to_string_lossy().into_owned();
    let (json, code) = run_json(&["orphans", "--in", &dir, "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["count"], 0, "{json}");
    let r = run(&["orphans", "--in", &dir]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(
        r.stdout.contains("No orphaned processes found")
            || r.stderr.contains("No orphaned processes found"),
        "{}{}",
        r.stdout,
        r.stderr
    );
    // a name filter that matches nothing leaves nothing too
    let (json, code) = run_json(&["orphans", "--by", "no-process-has-this-name", "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["count"], 0, "{json}");
}

/// A shell loop that uses a whole core and ends itself after two minutes, so a
/// test binary killed without running `Drop` cannot leave it spinning for long
#[cfg(unix)]
const SPIN: &str = "end=$(( $(date +%s) + 120 )); n=0; \
    while :; do n=$((n+1)); \
    if [ $((n % 50000)) -eq 0 ] && [ \"$(date +%s)\" -ge \"$end\" ]; then exit 0; fi; done";

/// A shell the test owns, killed and reaped on drop
#[cfg(unix)]
struct Shell {
    child: Child,
    _tmp: tempfile::TempDir,
    dir: PathBuf,
    _busy: Option<MutexGuard<'static, ()>>,
}

#[cfg(unix)]
impl Shell {
    /// Runs `script` in a fresh directory (holding the one-spinner lock if
    /// `spins`); returns once the script has printed `ready`
    fn run(script: &str, spins: bool) -> Self {
        let busy = spins.then(busy_lock);
        let (tmp, dir) = fresh_dir();
        let mut child = std::process::Command::new("sh")
            .args(["-c", script])
            .current_dir(&dir)
            .stdout(Stdio::piped())
            .stdin(Stdio::null())
            .spawn()
            .expect("spawn sh");
        let mut line = String::new();
        let mut out = BufReader::new(child.stdout.take().unwrap());
        let _ = out.read_line(&mut line);
        if line.trim() != "ready" {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the shell printed {line:?} instead of `ready`");
        }
        // keep reading end open so the shell never gets SIGPIPE
        std::mem::forget(out);
        let shell = Shell {
            child,
            _tmp: tmp,
            dir,
            _busy: busy,
        };
        if spins {
            // proc has no CPU reading for a process that has used no CPU yet
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                let (json, _) = run_json(&[
                    "stuck",
                    "--timeout",
                    "0",
                    "--in",
                    &shell.dir.to_string_lossy(),
                    "--json",
                ]);
                if lists(&json, "processes", shell.pid()) {
                    break;
                }
                assert!(Instant::now() < deadline, "never seen as stuck: {json}");
            }
        }
        shell
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn pid_arg(&self) -> String {
        self.pid().to_string()
    }

    fn ended_by_signal(&mut self, limit: Duration) -> Option<i32> {
        use std::os::unix::process::ExitStatusExt;
        let deadline = Instant::now() + limit;
        while Instant::now() < deadline {
            if let Ok(Some(status)) = self.child.try_wait() {
                return status.signal();
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        None
    }

    fn ended_within(&mut self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        while Instant::now() < deadline {
            if !matches!(self.child.try_wait(), Ok(None)) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }
}

#[cfg(unix)]
impl Drop for Shell {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A process whose parent has already exited, so init adopts it
#[cfg(unix)]
struct Orphan {
    pid: i32,
    /// Set once seen gone, so `drop` never signals the PID again (it may be reused)
    gone: std::cell::Cell<bool>,
    _tmp: tempfile::TempDir,
    dir: PathBuf,
    _busy: Option<MutexGuard<'static, ()>>,
}

#[cfg(unix)]
impl Orphan {
    fn sleeping() -> Self {
        Self::start("sleep 120 >/dev/null 2>&1 </dev/null & echo $!", false)
    }

    fn spinning() -> Self {
        Self::start(
            &format!("sh -c '{SPIN}' >/dev/null 2>&1 </dev/null & echo $!"),
            true,
        )
    }

    fn start(script: &str, spins: bool) -> Self {
        let busy = spins.then(busy_lock);
        let (tmp, dir) = fresh_dir();
        let out = std::process::Command::new("sh")
            .args(["-c", script])
            .current_dir(&dir)
            .stdin(Stdio::null())
            .output()
            .expect("spawn the orphan's parent");
        let pid: i32 = String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse()
            .expect("the shell prints the orphan's pid");
        let orphan = Orphan {
            pid,
            gone: std::cell::Cell::new(false),
            _tmp: tmp,
            dir,
            _busy: busy,
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        while orphan.ps("ppid=").as_deref() != Some("1") {
            assert!(Instant::now() < deadline, "the orphan was not reparented");
            std::thread::sleep(Duration::from_millis(50));
        }
        orphan
    }

    fn ps(&self, column: &str) -> Option<String> {
        let out = std::process::Command::new("ps")
            .args(["-o", column, "-p", &self.pid.to_string()])
            .output()
            .ok()?;
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    fn dir(&self) -> String {
        self.dir.to_string_lossy().into_owned()
    }

    /// Nobody but init reaps it: gone means `ps` shows nothing or a zombie
    fn gone_within(&self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        while Instant::now() < deadline {
            let state = self.ps("stat=").unwrap_or_default();
            if state.is_empty() || state.starts_with('Z') {
                self.gone.set(true);
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    fn alive(&self) -> bool {
        !self.gone_within(Duration::ZERO)
    }
}

#[cfg(unix)]
impl Drop for Orphan {
    fn drop(&mut self) {
        if self.gone.get() {
            return;
        }
        // SAFETY: the pid is the orphan this test started and it has not been
        // seen gone, so the PID is still its own
        unsafe {
            libc::kill(self.pid, libc::SIGKILL);
        }
    }
}

#[cfg(unix)]
#[test]
fn orphans_lists_the_orphan_with_its_cpu() {
    let orphan = Orphan::sleeping();
    let pid = orphan.pid as u32;
    let (json, code) = run_json(&["orphans", "--in", &orphan.dir(), "--json"]);
    assert_eq!(code, 0, "{json}");
    assert!(lists(&json, "processes", pid), "{json}");
    let r = run(&["orphans", "--in", &orphan.dir()]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains(&pid.to_string()), "{}", r.stdout);
    assert!(
        r.stdout.contains("Found 1 orphaned process")
            || r.stderr.contains("Found 1 orphaned process"),
        "{}{}",
        r.stdout,
        r.stderr
    );
    assert!(orphan.alive());
}

#[cfg(unix)]
#[test]
fn orphans_kill_dry_run_says_what_it_would_kill_and_kills_nothing() {
    let orphan = Orphan::sleeping();
    let pid = orphan.pid as u32;
    let (json, code) = run_json(&[
        "orphans",
        "--kill",
        "--dry-run",
        "--in",
        &orphan.dir(),
        "--json",
    ]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["action"], "orphans");
    assert_eq!(json["dry_run"], true);
    assert!(lists(&json, "processes", pid), "{json}");
    let r = run(&["orphans", "--kill", "--dry-run", "--in", &orphan.dir()]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    let says = "Dry run: would kill 1 orphaned process";
    assert!(
        r.stdout.contains(says) || r.stderr.contains(says),
        "{}{}",
        r.stdout,
        r.stderr
    );
    assert!(orphan.alive());
}

#[cfg(unix)]
#[test]
fn orphans_kill_without_yes_and_json_asks_for_yes_with_measured_cpu() {
    let orphan = Orphan::spinning();
    let pid = orphan.pid as u32;
    let (json, code) = run_json(&["orphans", "--kill", "--in", &orphan.dir(), "--json"]);
    assert_eq!(code, 4, "{json}");
    assert_eq!(json["needs_confirmation"], true, "{json}");
    assert_eq!(json["processes"][0]["pid"], pid, "{json}");
    let cpu = json["processes"][0]["cpu_percent"].as_f64().unwrap();
    assert!(cpu > MEASURED, "cpu_percent {cpu} was not measured: {json}");
    assert!(orphan.alive());
}

#[cfg(unix)]
#[test]
fn orphans_kill_yes_kills_the_orphan_and_says_so() {
    let orphan = Orphan::sleeping();
    let r = run(&["orphans", "--kill", "--yes", "--in", &orphan.dir()]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains("Killed 1 process"), "{}", r.stdout);
    assert!(orphan.gone_within(Duration::from_secs(10)));

    let second = Orphan::sleeping();
    let (json, code) = run_json(&[
        "orphans",
        "--kill",
        "--yes",
        "--in",
        &second.dir(),
        "--json",
    ]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["action"], "orphans");
    assert_eq!(json["succeeded_count"], 1, "{json}");
    assert!(second.gone_within(Duration::from_secs(10)));
}

// ---- unstick ----

/// The `NN.N` in the `NN.N% CPU` of the human line for this PID
fn cpu_in_line(stdout: &str, pid: u32) -> f64 {
    let line = stdout
        .lines()
        .find(|l| l.contains(&format!("[PID {pid}]")))
        .unwrap_or_else(|| panic!("no line for {pid}:\n{stdout}"));
    let before = line.split("% CPU").next().unwrap();
    before
        .rsplit(' ')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("no CPU figure in {line}"))
}

#[test]
fn unstick_with_a_target_that_is_gone_is_not_found() {
    // `--in` a directory of the test's own, so a reused PID is never touched
    let gone = Owned::listener().gone_pid();
    let (_tmp, own_dir) = fresh_dir();
    let own_dir = own_dir.to_string_lossy().into_owned();
    let r = run(&["unstick", &gone, "--yes", "--in", &own_dir]);
    assert_eq!(r.code, 2, "{}{}", r.stdout, r.stderr);
    let owned = Owned::listener();
    let r = run(&[
        "unstick",
        &owned.pid_arg(),
        "--yes",
        "--by",
        "no-process-has-this-name",
    ]);
    assert_eq!(r.code, 2, "{}{}", r.stdout, r.stderr);
}

#[test]
fn unstick_with_nothing_stuck_says_so() {
    let (_tmp, dir) = fresh_dir();
    let dir = dir.to_string_lossy().into_owned();
    let (json, code) = run_json(&["unstick", "--timeout", "0", "--in", &dir, "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["action"], "unstick");
    assert_eq!(json["success"], true);
    assert_eq!(json["found"], 0);
    let r = run(&["unstick", "--timeout", "0", "--in", &dir]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    let says = "No stuck processes found";
    assert!(
        r.stdout.contains(says) || r.stderr.contains(says),
        "{}{}",
        r.stdout,
        r.stderr
    );
}

#[test]
fn unstick_dry_run_shows_measured_cpu_and_attempts_nothing() {
    let mut busy = Owned::busy();
    let pid = busy.pid_arg();
    let r = run(&["unstick", &pid, "--dry-run"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    let cpu = cpu_in_line(&r.stdout, busy.pid());
    assert!(cpu > MEASURED, "cpu {cpu} was not measured:\n{}", r.stdout);
    assert!(
        r.stdout
            .contains("Dry run: Would attempt to unstick 1 process"),
        "{}",
        r.stdout
    );
    assert!(
        r.stdout
            .contains("Without --force: will only attempt recovery"),
        "{}",
        r.stdout
    );

    let r = run(&["unstick", &pid, "--dry-run", "--force"]);
    assert!(
        r.stdout
            .contains("With --force: will terminate if recovery fails"),
        "{}",
        r.stdout
    );

    let (json, code) = run_json(&["unstick", &pid, "--dry-run", "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["dry_run"], true);
    assert_eq!(json["found"], 1);
    assert_eq!(json["processes"][0]["outcome"], "would_attempt", "{json}");
    assert!(busy.alive());
}

#[test]
fn unstick_without_yes_and_json_asks_for_yes_with_measured_cpu() {
    let mut busy = Owned::busy();
    let (json, code) = run_json(&["unstick", &busy.pid_arg(), "--json"]);
    assert_eq!(code, 4, "{json}");
    assert_eq!(json["needs_confirmation"], true, "{json}");
    assert_eq!(json["processes"][0]["pid"], busy.pid(), "{json}");
    let cpu = json["processes"][0]["cpu_percent"].as_f64().unwrap();
    assert!(cpu > MEASURED, "cpu_percent {cpu} was not measured: {json}");
    assert!(busy.alive());
}

#[test]
fn unstick_leaves_a_process_that_is_not_stuck_alone() {
    let mut idle = Owned::listener();
    let pid = idle.pid_arg();
    let r = run(&["unstick", &pid, "--yes"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains("Target 1 process"), "{}", r.stdout);
    assert!(r.stdout.contains("not stuck"), "{}", r.stdout);
    assert!(r.stdout.contains("1 process was not stuck"), "{}", r.stdout);
    let (json, code) = run_json(&["unstick", &pid, "--yes", "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["not_stuck"], 1, "{json}");
    assert_eq!(json["processes"][0]["outcome"], "not_stuck", "{json}");
    assert_eq!(
        json["processes"][0]["name"].as_str().map(str::is_empty),
        Some(false)
    );
    assert!(idle.alive());
}

#[test]
fn unstick_without_force_acts_on_a_busy_process_by_what_the_os_allows() {
    let mut busy = Owned::busy();
    let (json, code) = run_json(&["unstick", &busy.pid_arg(), "--yes", "--json"]);
    if cfg!(unix) {
        // the listener dies on SIGINT, step 2
        assert_eq!(code, 0, "{json}");
        assert_eq!(json["terminated"], 1, "{json}");
        assert_eq!(json["processes"][0]["outcome"], "terminated", "{json}");
        assert!(busy.ended_within(Duration::from_secs(10)));
    } else {
        // Windows has nothing gentle to try, and acts only with --force
        assert_eq!(code, 1, "{json}");
        assert_eq!(json["still_stuck"], 1, "{json}");
        assert_eq!(json["success"], false, "{json}");
        assert!(busy.alive());
    }
}

#[test]
fn unstick_force_terminates_a_busy_process() {
    let mut busy = Owned::busy();
    let r = run(&["unstick", &busy.pid_arg(), "--yes", "--force"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains("terminated"), "{}", r.stdout);
    assert!(r.stdout.contains("1 process terminated"), "{}", r.stdout);
    assert!(busy.ended_within(Duration::from_secs(15)));
}

#[test]
fn unstick_without_a_target_finds_the_stuck_process_in_the_directory() {
    let mut busy = Owned::busy();
    let (json, code) = run_json(&[
        "unstick",
        "--timeout",
        "0",
        "--in",
        &busy.dir(),
        "--yes",
        "--force",
        "--json",
    ]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["found"], 1, "{json}");
    assert_eq!(json["terminated"], 1, "{json}");
    assert!(lists(&json, "processes", busy.pid()), "{json}");
    assert!(busy.ended_within(Duration::from_secs(15)));
}

#[cfg(unix)]
#[test]
fn unstick_recovers_a_process_that_calms_down_when_woken() {
    // SIGCONT makes the shell swap itself for an idle sleep
    let script = format!("trap 'exec sleep 120' CONT; echo ready; {SPIN}");
    let mut shell = Shell::run(&script, true);
    let r = run(&["unstick", &shell.pid_arg(), "--yes"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains("recovered"), "{}", r.stdout);
    assert!(r.stdout.contains("1 process recovered"), "{}", r.stdout);
    assert!(!shell.ended_within(Duration::ZERO));
    // one spinner at a time: the first must go before the second can start
    drop(shell);

    let mut again = Shell::run(&script, true);
    let (json, code) = run_json(&["unstick", &again.pid_arg(), "--yes", "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["recovered"], 1, "{json}");
    assert_eq!(json["processes"][0]["outcome"], "recovered", "{json}");
    assert!(!again.ended_within(Duration::ZERO));
}

#[cfg(unix)]
#[test]
fn unstick_without_force_leaves_a_process_that_ignores_sigint_still_stuck() {
    let script = format!("trap '' INT TERM; echo ready; {SPIN}");
    let mut shell = Shell::run(&script, true);
    let r = run(&["unstick", &shell.pid_arg(), "--yes"]);
    assert_eq!(r.code, 1, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains("still stuck"), "{}", r.stdout);
    assert!(
        r.stdout
            .contains("1 process still stuck (use --force to terminate)"),
        "{}",
        r.stdout
    );
    assert!(!shell.ended_within(Duration::ZERO));
}

#[cfg(unix)]
#[test]
fn unstick_force_goes_on_to_sigkill_for_a_process_that_ignores_everything_gentle() {
    let script = format!("trap '' INT TERM; echo ready; {SPIN}");
    let mut shell = Shell::run(&script, true);
    let (json, code) = run_json(&["unstick", &shell.pid_arg(), "--yes", "--force", "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["terminated"], 1, "{json}");
    assert_eq!(
        shell.ended_by_signal(Duration::from_secs(5)),
        Some(libc::SIGKILL)
    );
}
