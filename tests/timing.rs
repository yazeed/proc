//! Timing tests: commands that wait must stop waiting when they should.
//!
//! These live in their own test binary because cargo runs test binaries one
//! after another, so dozens of other proc runs can't slow these down. The limits
//! still separate the fixed behaviour from the old one, which took 5 seconds or
//! more, or never returned.
#![cfg(unix)]

use assert_cmd::Command;
use serde_json::Value;
use std::process::{Child, Command as StdCommand};
use std::time::{Duration, Instant};

/// A child process that is always killed and reaped when the test ends.
struct TestChild(Child);

impl TestChild {
    /// A sleep that exits after `secs`. The test doesn't reap it until drop, so
    /// once it exits it is a zombie: gone, but its PID is still taken.
    fn exiting_after(secs: &str) -> Self {
        TestChild(StdCommand::new("sleep").arg(secs).spawn().unwrap())
    }

    /// A process that ignores SIGTERM, so only SIGKILL ends it.
    fn ignoring_sigterm() -> Self {
        let child = StdCommand::new("sh")
            .args(["-c", "trap '' TERM; exec sleep 300"])
            .spawn()
            .unwrap();
        // let the shell install the trap before anyone signals it
        std::thread::sleep(Duration::from_millis(300));
        TestChild(child)
    }

    fn pid(&self) -> String {
        self.0.id().to_string()
    }
}

impl Drop for TestChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Makes the command's child start with SIGINT at its default. A test run
/// started from a background job (a trailing `&`, as cargo may be run) has
/// SIGINT ignored, every child inherits that, and a shell cannot take it back:
/// measured, `unstick` then sees SIGINT do nothing to a busy loop and reports it
/// `still_stuck`. Step 2 of `unstick` needs the default, so a test that expects
/// SIGINT to end its child sets it.
fn default_sigint(command: &mut StdCommand) -> &mut StdCommand {
    use std::os::unix::process::CommandExt;
    // SAFETY: only signal(2), which is async-signal-safe, runs between fork and exec
    unsafe {
        command.pre_exec(|| {
            libc::signal(libc::SIGINT, libc::SIG_DFL);
            Ok(())
        });
    }
    command
}

/// Run proc with a hard time limit; returns the JSON, the exit code (None if
/// proc had to be killed at the limit) and how long it took in seconds.
fn run_json_timed(args: &[&str], limit_secs: u64) -> (Value, Option<i32>, f64) {
    let started = Instant::now();
    let output = Command::cargo_bin("proc")
        .unwrap()
        .args(args)
        .timeout(Duration::from_secs(limit_secs))
        .output()
        .unwrap();
    let took = started.elapsed().as_secs_f64();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json = serde_json::from_str(&stdout).unwrap_or(Value::Null);
    (json, output.status.code(), took)
}

#[test]
fn wait_timeout_is_not_rounded_up_to_the_interval() {
    // --timeout 1 with the default 5-second interval used to take 5 seconds;
    // it now takes about 1.1
    let child = TestChild::exiting_after("300");
    let (json, code, took) =
        run_json_timed(&["wait", &child.pid(), "--timeout", "1", "--json"], 20);
    assert_eq!(code, Some(1), "{json}");
    assert_eq!(json["timed_out"], true);
    assert!(took < 4.0, "took {took:.1}s for --timeout 1");
}

#[test]
fn wait_returns_soon_after_the_process_exits() {
    // The child becomes a zombie when it exits (this test is its parent and
    // hasn't reaped it), which must count as exited. It used to take 5 seconds
    // to notice; it now takes about 1.3
    let child = TestChild::exiting_after("1");
    let (json, code, took) = run_json_timed(&["wait", &child.pid(), "--json"], 20);
    assert_eq!(code, Some(0), "{json}");
    assert_eq!(json["success"], true);
    assert!(
        took < 4.0,
        "took {took:.1}s to notice a 1-second process exit"
    );
}

#[test]
fn stop_waits_for_all_processes_together_and_never_hangs() {
    // Both ignore SIGTERM, so both need SIGKILL after the 3-second timeout.
    // Handled one after the other they took at least 6 seconds, and the wait for
    // an unreaped (zombie) process never ended; together they take about 3
    let first = TestChild::ignoring_sigterm();
    let second = TestChild::ignoring_sigterm();
    let targets = format!("{},{}", first.pid(), second.pid());
    let (json, code, took) =
        run_json_timed(&["stop", &targets, "--yes", "--timeout", "3", "--json"], 30);
    assert_eq!(
        code,
        Some(0),
        "stop hung or failed after {took:.1}s: {json}"
    );
    assert_eq!(json["succeeded_count"], 2, "{json}");
    assert!(
        took < 5.5,
        "took {took:.1}s for two processes with --timeout 3"
    );
}

#[test]
fn unstick_sees_a_spinning_process_as_stuck() {
    // CPU used to be read over a few milliseconds, which reads about 0% for
    // anything, so a targeted unstick called every process "not stuck". A
    // shell busy loop is stuck by any measure, and SIGINT (step 2) ends it.
    let spinning = TestChild(
        default_sigint(StdCommand::new("sh").args(["-c", "while :; do :; done"]))
            .spawn()
            .unwrap(),
    );
    std::thread::sleep(Duration::from_millis(300));
    let (json, code, took) = run_json_timed(&["unstick", &spinning.pid(), "--yes", "--json"], 30);
    assert_eq!(code, Some(0), "after {took:.1}s: {json}");
    assert_eq!(json["not_stuck"], 0, "{json}");
    assert_eq!(json["terminated"], 1, "{json}");
}

#[test]
fn list_measures_cpu_once_for_all_processes() {
    // Real CPU readings need a 0.2 s window. One window covers every process
    // (about 0.3 s in all); one per process would take minutes on a machine
    // with a thousand processes
    let (json, code, took) = run_json_timed(&["list", "--json"], 60);
    assert_eq!(code, Some(0), "{json}");
    let count = json["count"].as_u64().unwrap();
    assert!(took < 2.0, "list of {count} processes took {took:.1}s");
}

#[test]
fn the_missing_yes_error_lists_processes_with_measured_cpu() {
    // The error returned for `kill --json` without --yes carries the same
    // process list as --dry-run, so its cpu_percent must be measured too. An
    // idle sleeper reads 0 either way; a busy loop tells the two apart
    let spinning = TestChild(
        StdCommand::new("sh")
            .args(["-c", "while :; do :; done"])
            .spawn()
            .unwrap(),
    );
    std::thread::sleep(Duration::from_millis(300));
    let (json, code, took) = run_json_timed(&["kill", &spinning.pid(), "--json"], 30);
    assert_eq!(code, Some(4), "after {took:.1}s: {json}");
    assert_eq!(json["needs_confirmation"], true, "{json}");
    let cpu = json["processes"][0]["cpu_percent"].as_f64().unwrap();
    // unmeasured reads ~0; a full core measured can read under 50 on a loaded
    // CI runner, so 20 tells the two apart without flaking
    assert!(cpu > 20.0, "cpu_percent {cpu} was not measured: {json}");
}

/// Kills and reaps a proc that the test started, whatever happens
struct Proc(Child);

impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Wait up to `limit` for the child to end; its exit code if it did
fn ended_within(child: &mut Proc, limit: Duration) -> Option<Option<i32>> {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if let Some(status) = child.0.try_wait().unwrap() {
            return Some(status.code());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

#[test]
fn watch_json_ends_soon_after_its_reader_leaves() {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;

    // `proc watch node -j -n 30 | head -n1`. The broken pipe used to be noticed
    // by the next write, a whole 30-second refresh later
    let target = TestChild::exiting_after("300");
    let mut watch = Proc(
        StdCommand::new(env!("CARGO_BIN_EXE_proc"))
            .args(["watch", &target.pid(), "--json", "-n", "30"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut reader = BufReader::new(watch.0.stdout.take().unwrap());
    let mut first = String::new();
    reader.read_line(&mut first).unwrap();
    assert!(first.contains("\"action\":\"watch\""), "{first}");

    let left = Instant::now();
    drop(reader);
    let code = ended_within(&mut watch, Duration::from_secs(5))
        .unwrap_or_else(|| panic!("still running {:.1}s after its reader left", 5.0));
    assert_eq!(code, Some(0), "a reader that left is not an error");
    assert!(
        left.elapsed() < Duration::from_secs(5),
        "took {:?} to notice",
        left.elapsed()
    );
}

#[test]
fn wait_keeps_waiting_when_its_reader_leaves_and_ends_with_its_own_code() {
    use std::process::Stdio;

    // Unlike a watch, wait has a result: its exit code says whether the
    // processes ended (0) or it timed out. So a reader that left changes
    // nothing: it waits for the process and exits 0 when it is gone.
    let target = TestChild::exiting_after("2");
    let (reader, writer) = std::io::pipe().unwrap();
    drop(reader);
    let started = Instant::now();
    let mut wait = Proc(
        StdCommand::new(env!("CARGO_BIN_EXE_proc"))
            .args(["wait", &target.pid()])
            .stdout(Stdio::from(writer))
            .spawn()
            .unwrap(),
    );
    assert_eq!(
        ended_within(&mut wait, Duration::from_secs(20)),
        Some(Some(0))
    );
    assert!(
        started.elapsed() >= Duration::from_millis(1500),
        "wait stopped after {:?}, before the process had ended",
        started.elapsed()
    );

    // and a timeout is still a timeout
    let target = TestChild::exiting_after("300");
    let (reader, writer) = std::io::pipe().unwrap();
    drop(reader);
    let mut wait = Proc(
        StdCommand::new(env!("CARGO_BIN_EXE_proc"))
            .args(["wait", &target.pid(), "--timeout", "1"])
            .stdout(Stdio::from(writer))
            .spawn()
            .unwrap(),
    );
    assert_eq!(
        ended_within(&mut wait, Duration::from_secs(20)),
        Some(Some(1))
    );
}
