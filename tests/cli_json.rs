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
