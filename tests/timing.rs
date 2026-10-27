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
