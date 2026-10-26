//! JSON output contract tests: every command prints exactly one JSON document,
//! `action` is the canonical command name, and `success: false` means a non-zero exit.

use assert_cmd::Command;
use serde_json::Value;

/// A PID that will not exist on any test machine
const MISSING_PID: &str = "99999999";

fn proc_cmd() -> Command {
    Command::cargo_bin("proc").unwrap()
}

/// Run proc and parse stdout as exactly one JSON document.
fn run_json(args: &[&str]) -> (Value, i32) {
    let output = proc_cmd().args(args).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not a single JSON document ({e}):\n{stdout}"));
    (json, output.status.code().unwrap())
}

#[test]
fn usage_error_exits_invalid_input_with_json() {
    let (json, code) = run_json(&["kill", MISSING_PID, "--bogus-flag", "--json"]);
    assert_eq!(code, 4);
    assert_eq!(json["action"], "kill");
    assert_eq!(json["success"], false);
    assert_eq!(json["exit_code"], 4);
    assert!(json["error"].as_str().unwrap().contains("--bogus-flag"));
}

#[test]
fn usage_error_without_json_goes_to_stderr() {
    let output = proc_cmd()
        .args(["kill", MISSING_PID, "--bogus-flag"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(4));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unexpected argument"));
}

#[test]
fn alias_reports_canonical_action() {
    let (json, code) = run_json(&["k", MISSING_PID, "--yes", "--json"]);
    assert_eq!(code, 2);
    assert_eq!(json["action"], "kill");
}

#[test]
fn inspect_is_an_alias_of_info() {
    let (json, code) = run_json(&["inspect", MISSING_PID, "--json"]);
    assert_eq!(code, 2);
    assert_eq!(json["action"], "info");
}

#[test]
fn short_flag_cluster_enables_json_errors() {
    let (json, code) = run_json(&["kill", MISSING_PID, "-yj"]);
    assert_eq!(code, 2);
    assert_eq!(json["success"], false);
}

#[test]
fn why_not_found_prints_json_error() {
    let (json, code) = run_json(&["why", MISSING_PID, "--json"]);
    assert_eq!(code, 2);
    assert_eq!(json["action"], "why");
    assert_eq!(json["success"], false);
}

#[test]
fn on_multi_target_not_found_prints_json_error() {
    let (json, code) = run_json(&["on", &format!("{MISSING_PID},{MISSING_PID}1"), "--json"]);
    assert_eq!(code, 2);
    assert_eq!(json["action"], "on");
    assert_eq!(json["success"], false);
}

#[test]
fn in_missing_directory_is_invalid_input() {
    let (json, code) = run_json(&["in", "/nonexistent/proc-test-dir", "--json"]);
    assert_eq!(code, 4);
    assert_eq!(json["action"], "in");
}

#[test]
fn unknown_status_filter_is_invalid_input() {
    // A typo must not silently match every process
    for cmd in ["list", "tree"] {
        let (json, code) = run_json(&[cmd, "--status", "stoped", "--json"]);
        assert_eq!(code, 4, "{cmd}: {json}");
        assert_eq!(json["action"], cmd);
        assert!(json["error"].as_str().unwrap().contains("stoped"));
    }
    let (_, code) = run_json(&["list", "--status", "STOP", "--json"]);
    assert_eq!(code, 0, "aliases and any case still work");
}

#[test]
fn no_process_has_been_up_longer_than_the_machine() {
    // A start time that can't be read must not turn into "up since 1970"
    let boot_uptime = sysinfo::System::uptime();
    let (json, code) = run_json(&["list", "--json"]);
    assert_eq!(code, 0);
    for p in json["processes"].as_array().unwrap() {
        if let Some(uptime) = p["uptime_secs"].as_u64() {
            assert!(
                uptime <= boot_uptime + 5,
                "{} (PID {}) reports {uptime}s, but the system has been up {boot_uptime}s",
                p["name"],
                p["pid"]
            );
        }
    }
}

#[test]
fn watch_rejects_intervals_it_cannot_use() {
    // nan, inf and negatives used to panic; 0 refreshed in a busy loop
    for bad in ["nan", "inf", "-1", "0"] {
        let arg = format!("--interval={bad}");
        let (json, code) = run_json(&["watch", &arg, "--json"]);
        assert_eq!(code, 4, "--interval {bad}: {json}");
        assert_eq!(json["success"], false);
    }
}

#[test]
fn a_colon_target_must_be_a_real_port() {
    // ":70000" used to fall through to a name search and exit 2 (not found)
    let cases: &[&[&str]] = &[
        &["on", ":70000", "--json"],
        &["why", ":abc", "--json"],
        &["info", ":0", "--json"],
        &["tree", ":99999", "--json"],
        &["watch", ":3000x", "--json"],
        &["kill", ":3000,:70000", "--dry-run", "--json"],
        &["free", ":65536", "--dry-run", "--json"],
    ];
    for args in cases {
        let (json, code) = run_json(args);
        assert_eq!(code, 4, "{args:?}: {json}");
        assert!(
            json["error"].as_str().unwrap().contains("not a port"),
            "{args:?}: {json}"
        );
    }
}

#[test]
fn ports_rejects_a_range_it_cannot_use() {
    // These used to be ignored, listing every port as if no range was given
    for bad in ["abc", "70000-80000", "9000-8000", "3000"] {
        let (json, code) = run_json(&["ports", "--range", bad, "--json"]);
        assert_eq!(code, 4, "--range {bad}: {json}");
        assert_eq!(json["success"], false);
    }
    let (json, code) = run_json(&["ports", "--range", "1-65535", "--json"]);
    assert_eq!(code, 0, "{json}");
}

#[test]
fn info_not_found_exits_not_found() {
    let (json, code) = run_json(&["info", MISSING_PID, "--json"]);
    assert_eq!(code, 2);
    assert_eq!(json["action"], "info");
    assert_eq!(json["success"], false);
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::process::{Child, Command as StdCommand};

    /// A `sleep` child that is always killed when the test ends.
    struct Sleeper(Child);

    impl Sleeper {
        fn spawn() -> Self {
            Sleeper(StdCommand::new("sleep").arg("300").spawn().unwrap())
        }

        fn pid(&self) -> String {
            self.0.id().to_string()
        }
    }

    impl Drop for Sleeper {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    impl Sleeper {
        /// A child that exits after `secs`. The test doesn't reap it until drop,
        /// so once it exits it is a zombie: gone, but its PID is still taken.
        fn exiting_after(secs: &str) -> Self {
            Sleeper(StdCommand::new("sleep").arg(secs).spawn().unwrap())
        }

        /// A child that ignores SIGTERM, so only SIGKILL ends it.
        fn ignoring_sigterm() -> Self {
            let child = StdCommand::new("sh")
                .args(["-c", "trap '' TERM; exec sleep 300"])
                .spawn()
                .unwrap();
            // let the shell install the trap before anyone signals it
            std::thread::sleep(std::time::Duration::from_millis(300));
            Sleeper(child)
        }
    }

    /// Run proc with a hard time limit; returns the JSON, the exit code
    /// (None if proc had to be killed at the limit) and how long it took.
    fn run_json_timed(args: &[&str], limit_secs: u64) -> (Value, Option<i32>, f64) {
        let started = std::time::Instant::now();
        let output = proc_cmd()
            .args(args)
            .timeout(std::time::Duration::from_secs(limit_secs))
            .output()
            .unwrap();
        let took = started.elapsed().as_secs_f64();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let json = serde_json::from_str(&stdout).unwrap_or(Value::Null);
        (json, output.status.code(), took)
    }

    #[test]
    fn dry_run_reports_own_action() {
        let sleeper = Sleeper::spawn();
        for action in ["kill", "stop", "freeze", "thaw"] {
            let (json, code) = run_json(&[action, &sleeper.pid(), "--dry-run", "--json"]);
            assert_eq!(code, 0, "{action}");
            assert_eq!(json["action"], action);
            assert_eq!(json["dry_run"], true, "{action}");
            assert_eq!(json["count"], 1, "{action}");
        }
    }

    #[test]
    fn user_is_reported_by_name() {
        let sleeper = Sleeper::spawn();
        let id = StdCommand::new("id").arg("-un").output().unwrap();
        let expected = String::from_utf8(id.stdout).unwrap();

        let (json, code) = run_json(&["info", &sleeper.pid(), "--json"]);
        assert_eq!(code, 0);
        assert_eq!(json["processes"][0]["user"], expected.trim());
    }

    #[test]
    fn uptime_is_reported_in_seconds() {
        let sleeper = Sleeper::spawn();

        let (json, code) = run_json(&["info", &sleeper.pid(), "--json"]);
        assert_eq!(code, 0);
        let process = &json["processes"][0];
        let uptime = process["uptime_secs"].as_u64().unwrap();
        let start = process["start_time"].as_u64().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        // The sleeper was just spawned, and uptime agrees with start_time
        assert!(uptime < 60, "uptime_secs = {uptime}");
        assert!(now.saturating_sub(start).abs_diff(uptime) <= 2);
    }

    #[test]
    fn every_process_view_reports_uptime() {
        let sleeper = Sleeper::spawn();
        let pid = sleeper.pid();
        let has_uptime = |v: &Value, what: &str| {
            assert!(v["uptime_secs"].as_u64().is_some(), "{what}: {v}");
        };

        let (tree, _) = run_json(&["tree", &pid, "--json"]);
        has_uptime(&tree["tree"][0], "tree node");

        // Ancestors owned by other users (PID 1 on macOS) have no readable
        // start time, so check the sleeper's own entry in each chain
        let own_entry = |chain: &Value| -> Value {
            chain
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["pid"].as_u64() == Some(u64::from(sleeper.0.id())))
                .cloned()
                .unwrap_or_else(|| panic!("sleeper not in chain: {chain}"))
        };

        let (ancestors, _) = run_json(&["tree", &pid, "--ancestors", "--json"]);
        has_uptime(
            &own_entry(&ancestors["ancestry"][0]["chain"]),
            "tree --ancestors chain entry",
        );

        let (why, _) = run_json(&["why", &pid, "--json"]);
        has_uptime(&why["results"][0]["process"], "why process");
        has_uptime(
            &own_entry(&why["results"][0]["ancestry"]),
            "why ancestry entry",
        );

        let (unstick, _) = run_json(&["unstick", &pid, "--dry-run", "--json"]);
        has_uptime(&unstick["processes"][0], "unstick process");

        let (on, _) = run_json(&["on", &pid, "--json"]);
        has_uptime(&on["process"], "on <pid> process");
    }

    #[test]
    fn on_multi_target_prints_one_document() {
        let first = Sleeper::spawn();
        let second = Sleeper::spawn();
        let targets = format!("{},{},{}", first.pid(), second.pid(), MISSING_PID);
        let (json, code) = run_json(&["on", &targets, "--json"]);
        assert_eq!(code, 0);
        assert_eq!(json["action"], "on");
        assert_eq!(json["count"], 2);
        assert_eq!(json["results"][0]["target"], first.pid());
        assert_eq!(json["not_found"][0], MISSING_PID);
    }

    #[test]
    fn why_multi_target_prints_one_document() {
        let first = Sleeper::spawn();
        let second = Sleeper::spawn();
        let targets = format!("{},{}", first.pid(), second.pid());
        let (json, code) = run_json(&["why", &targets, "--json"]);
        assert_eq!(code, 0);
        assert_eq!(json["count"], 2);
    }

    #[test]
    fn thaw_reports_thaw_action() {
        let sleeper = Sleeper::spawn();
        let (frozen, _) = run_json(&["freeze", &sleeper.pid(), "--yes", "--json"]);
        assert_eq!(frozen["action"], "freeze");
        let (json, code) = run_json(&["thaw", &sleeper.pid(), "--yes", "--json"]);
        assert_eq!(code, 0);
        assert_eq!(json["action"], "thaw");
        assert_eq!(json["succeeded_count"], 1);
    }

    #[test]
    fn wait_timeout_exits_non_zero() {
        let sleeper = Sleeper::spawn();
        let (json, code) = run_json(&[
            "wait",
            &sleeper.pid(),
            "--timeout",
            "1",
            "--interval",
            "1",
            "--json",
        ]);
        assert_eq!(code, 1);
        assert_eq!(json["success"], false);
        assert_eq!(json["timed_out"], true);
        assert_eq!(json["still_running"][0]["pid"], sleeper.0.id());
    }

    #[test]
    fn wait_timeout_is_not_rounded_up_to_the_interval() {
        // --timeout 1 with the default 5-second interval used to take 5 seconds
        let sleeper = Sleeper::spawn();
        let (json, code, took) =
            run_json_timed(&["wait", &sleeper.pid(), "--timeout", "1", "--json"], 20);
        assert_eq!(code, Some(1), "{json}");
        assert_eq!(json["timed_out"], true);
        assert!(took < 3.0, "took {took:.1}s for --timeout 1");
    }

    #[test]
    fn wait_returns_soon_after_the_process_exits() {
        // The child becomes a zombie when it exits (this test is its parent and
        // hasn't reaped it), which must count as exited
        let sleeper = Sleeper::exiting_after("1");
        let (json, code, took) = run_json_timed(&["wait", &sleeper.pid(), "--json"], 20);
        assert_eq!(code, Some(0), "{json}");
        assert_eq!(json["success"], true);
        assert!(
            took < 3.0,
            "took {took:.1}s to notice a 1-second process exit"
        );
    }

    #[test]
    fn stop_waits_for_all_processes_together_and_never_hangs() {
        // Both ignore SIGTERM, so both need SIGKILL after the 1-second timeout.
        // They used to be handled one after the other, and the second wait for
        // an unreaped (zombie) process never ended.
        let first = Sleeper::ignoring_sigterm();
        let second = Sleeper::ignoring_sigterm();
        let targets = format!("{},{}", first.pid(), second.pid());
        let (json, code, took) =
            run_json_timed(&["stop", &targets, "--yes", "--timeout", "1", "--json"], 20);
        assert_eq!(
            code,
            Some(0),
            "stop hung or failed after {took:.1}s: {json}"
        );
        assert_eq!(json["succeeded_count"], 2, "{json}");
        assert!(
            took < 4.0,
            "took {took:.1}s for two processes with --timeout 1"
        );
    }

    #[test]
    fn zombies_are_listed() {
        let sleeper = Sleeper::exiting_after("0");
        std::thread::sleep(std::time::Duration::from_millis(500));
        let (json, code) = run_json(&["list", "--status", "zombie", "--json"]);
        assert_eq!(code, 0);
        let found = json["processes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["pid"].as_u64() == Some(u64::from(sleeper.0.id())));
        assert!(found, "zombie {} not listed: {json}", sleeper.pid());
    }

    #[test]
    fn tree_and_why_status_is_lowercase() {
        let sleeper = Sleeper::spawn();
        let (tree, _) = run_json(&["tree", &sleeper.pid(), "--json"]);
        let status = tree["tree"][0]["status"].as_str().unwrap();
        assert_eq!(status, status.to_lowercase());

        let (why, _) = run_json(&["why", &sleeper.pid(), "--json"]);
        let status = why["results"][0]["process"]["status"].as_str().unwrap();
        assert_eq!(status, status.to_lowercase());
    }

    #[test]
    fn stop_rejects_an_unknown_signal_even_on_dry_run() {
        let mut sleeper = Sleeper::spawn();
        for extra in [&["--dry-run"][..], &["--yes"][..]] {
            let mut args = vec!["stop", "--signal", "9", "--json"];
            let pid = sleeper.pid();
            args.push(&pid);
            args.extend_from_slice(extra);
            let (json, code) = run_json(&args);
            assert_eq!(code, 4, "{extra:?}: {json}");
            assert_eq!(json["action"], "stop");
            assert!(json["error"].as_str().unwrap().contains("Unknown signal"));
        }
        assert!(
            sleeper.0.try_wait().unwrap().is_none(),
            "sleeper was signalled"
        );
    }

    #[test]
    fn in_finds_processes_in_a_deleted_directory() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().canonicalize().unwrap();
        let dir_str = dir_path.to_str().unwrap().to_string();
        let sleeper = Sleeper(
            StdCommand::new("sleep")
                .arg("300")
                .current_dir(&dir_path)
                .spawn()
                .unwrap(),
        );
        dir.close().unwrap();
        assert!(!dir_path.exists());

        for args in [
            vec!["in", dir_str.as_str(), "--json"],
            vec!["list", "--in", dir_str.as_str(), "--json"],
        ] {
            let (json, code) = run_json(&args);
            assert_eq!(code, 0, "{args:?}: {json}");
            let found = json["processes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["pid"].as_u64() == Some(u64::from(sleeper.0.id())))
                .unwrap_or_else(|| panic!("{args:?} missed the sleeper: {json}"));
            // Linux reports "<dir> (deleted)"; proc shows the plain path everywhere
            assert_eq!(found["cwd"], dir_str.as_str(), "{args:?}");
        }
    }

    #[test]
    fn orphans_kill_prints_one_document() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().canonicalize().unwrap();
        let dir_str = dir_path.to_str().unwrap();

        // The shell exits immediately, so the backgrounded sleep is reparented to PID 1
        StdCommand::new("sh")
            .args(["-c", "sleep 300 &"])
            .current_dir(&dir_path)
            .status()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));

        let (found, _) = run_json(&["orphans", "--in", dir_str, "--json"]);
        if found["count"] == 0 {
            // Some init systems (e.g. a subreaper) adopt the child instead of PID 1
            return;
        }

        let (json, code) = run_json(&["orphans", "--in", dir_str, "--kill", "--yes", "--json"]);
        assert_eq!(code, 0);
        assert_eq!(json["action"], "orphans");
        assert_eq!(json["succeeded_count"], 1);
        assert_eq!(json["failed_count"], 0);
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;

    #[test]
    fn list_shows_processes_not_their_threads() {
        // Keep an extra thread alive so this test process has more than one task
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let worker = std::thread::spawn(move || rx.recv().ok());

        let me = std::process::id();
        let thread_ids: Vec<u64> = std::fs::read_dir("/proc/self/task")
            .unwrap()
            .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
            .filter(|tid| *tid != u64::from(me))
            .collect();
        assert!(!thread_ids.is_empty(), "test needs a second thread");

        let (json, code) = run_json(&["list", "--json"]);
        assert_eq!(code, 0);
        let pids: Vec<u64> = json["processes"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|p| p["pid"].as_u64())
            .collect();
        assert!(
            pids.contains(&u64::from(me)),
            "the test process itself is listed"
        );
        for tid in &thread_ids {
            assert!(!pids.contains(tid), "thread {tid} was listed as a process");
        }

        tx.send(()).unwrap();
        worker.join().unwrap();
    }
}

#[cfg(windows)]
mod windows {
    use super::*;

    #[test]
    fn stop_signal_is_rejected_on_windows() {
        // Windows has no signals; --signal must not quietly become a normal stop
        let pid = std::process::id().to_string();
        let (json, code) = run_json(&["stop", &pid, "--signal", "HUP", "--dry-run", "--json"]);
        assert_eq!(code, 4, "{json}");
        assert_eq!(json["action"], "stop");
        assert!(json["error"].as_str().unwrap().contains("Windows"));
    }
}
