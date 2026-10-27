//! A port held by several processes, driven through the real binary.
//!
//! Two sockets can carry one port number: one bound to `127.0.0.1`, one to
//! `[::1]`. The tests start a copy of the listener example for each (the
//! second with `--v6 --port <n>`), so every command that takes `:<port>` must
//! see both holders. Both are killed and reaped on drop; destructive commands
//! only run against them.

use assert_cmd::Command;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

fn proc_cmd() -> Command {
    let mut cmd = Command::cargo_bin("proc").unwrap();
    cmd.env("NO_COLOR", "1");
    cmd
}

/// Run proc and parse stdout as exactly one JSON document, with the exit code.
fn run_json(args: &[&str]) -> (Value, i32) {
    let output = proc_cmd().args(args).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not a single JSON document ({e}):\n{stdout}"));
    (json, output.status.code().unwrap())
}

/// Run proc and return its stdout and exit code.
fn run_text(args: &[&str]) -> (String, i32) {
    let output = proc_cmd().args(args).output().unwrap();
    (
        String::from_utf8(output.stdout).unwrap(),
        output.status.code().unwrap(),
    )
}

/// `target/debug/examples/listener`, found relative to this test binary
/// (`target/debug/deps/cmd_port_holders-<hash>`)
fn listener_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("examples");
    path.push(format!("listener{}", std::env::consts::EXE_SUFFIX));
    assert!(path.exists(), "build the example first: {}", path.display());
    path
}

/// One listener the test started. Killed and reaped on drop.
struct Holder {
    child: Child,
    port: u16,
}

impl Holder {
    fn start(args: &[&str]) -> Self {
        let mut child = std::process::Command::new(listener_path())
            .args(args)
            .stdout(Stdio::piped())
            .stdin(Stdio::null())
            .spawn()
            .expect("spawn the listener example");
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        let read = stdout.read_line(&mut line);
        let port = read
            .ok()
            .and_then(|_| line.trim().strip_prefix("PORT "))
            .and_then(|p| p.parse().ok());
        let Some(port) = port else {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the listener printed {line:?} instead of `PORT <n>`");
        };
        Holder { child, port }
    }

    fn pid(&self) -> u32 {
        self.child.id()
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

    fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for Holder {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Two processes holding one port number through two sockets
struct Pair {
    v4: Holder,
    v6: Holder,
}

impl Pair {
    fn start() -> Self {
        let v4 = Holder::start(&[]);
        let v6 = Holder::start(&["--v6", "--port", &v4.port.to_string()]);
        assert_eq!(v4.port, v6.port, "both listeners hold one port number");
        Pair { v4, v6 }
    }

    fn target(&self) -> String {
        format!(":{}", self.v4.port)
    }

    fn pids(&self) -> Vec<u64> {
        let mut pids = vec![u64::from(self.v4.pid()), u64::from(self.v6.pid())];
        pids.sort_unstable();
        pids
    }
}

/// The sorted PIDs of the processes in the JSON array `value`
fn pids_in(value: &Value) -> Vec<u64> {
    let mut pids: Vec<u64> = value
        .as_array()
        .unwrap_or_else(|| panic!("not an array: {value}"))
        .iter()
        .map(|p| p["pid"].as_u64().expect("a pid"))
        .collect();
    pids.sort_unstable();
    pids
}

#[test]
fn on_lists_every_holder_and_keeps_process_as_the_first() {
    let pair = Pair::start();
    let (json, code) = run_json(&["on", &pair.target(), "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["query_type"], "port_to_process", "{json}");
    assert_eq!(pids_in(&json["processes"]), pair.pids(), "{json}");
    // existing scripts read `process`: it is the first holder
    assert_eq!(
        json["process"]["pid"], json["processes"][0]["pid"],
        "{json}"
    );
    assert!(pair
        .pids()
        .contains(&json["process"]["pid"].as_u64().unwrap()));
}

#[test]
fn on_prints_every_holder() {
    let pair = Pair::start();
    let (out, code) = run_text(&["on", &pair.target()]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains(&format!("Port {} is used by 2 processes:", pair.v4.port)),
        "{out}"
    );
    for pid in pair.pids() {
        assert!(
            out.contains(&format!("(PID {pid})")),
            "{pid} missing: {out}"
        );
    }
}

#[test]
fn on_several_targets_json_has_a_result_per_holder() {
    let pair = Pair::start();
    let other = Holder::start(&[]);
    let other_target = format!(":{}", other.port);
    let (json, code) = run_json(&["on", &pair.target(), &other_target, "--json"]);
    assert_eq!(code, 0, "{json}");
    let results = json["results"].as_array().unwrap();
    let on_pair = results
        .iter()
        .filter(|r| r["target"] == pair.target().as_str())
        .count();
    assert_eq!(on_pair, 2, "{json}");
    assert_eq!(json["count"], 3, "{json}");
}

#[test]
fn free_dry_run_lists_every_holder() {
    let mut pair = Pair::start();
    let (json, code) = run_json(&["free", &pair.target(), "--dry-run", "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(pids_in(&json["processes"]), pair.pids(), "{json}");
    let (out, _) = run_text(&["free", &pair.target(), "--dry-run"]);
    assert!(
        out.contains("Dry run: would kill 2 processes to free 1 port"),
        "{out}"
    );
    assert!(
        pair.v4.alive() && pair.v6.alive(),
        "a dry run kills nothing"
    );
}

#[test]
fn free_kills_every_holder_and_reports_the_port_freed() {
    let mut pair = Pair::start();
    let (json, code) = run_json(&["free", &pair.target(), "--yes", "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["success"], true, "{json}");
    assert_eq!(json["results"][0]["port"], pair.v4.port);
    assert_eq!(json["results"][0]["freed"], true, "{json}");
    assert!(pair.v4.ended_within(Duration::from_secs(5)), "{json}");
    assert!(pair.v6.ended_within(Duration::from_secs(5)), "{json}");
}

#[test]
fn kill_and_stop_by_port_reach_every_holder() {
    let mut pair = Pair::start();
    for action in ["kill", "stop"] {
        let (json, code) = run_json(&[action, &pair.target(), "--dry-run", "--json"]);
        assert_eq!(code, 0, "{action}: {json}");
        assert_eq!(pids_in(&json["processes"]), pair.pids(), "{action}: {json}");
    }
    let (json, code) = run_json(&["kill", &pair.target(), "--yes", "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(pids_in(&json["succeeded"]), pair.pids(), "{json}");
    assert!(pair.v4.ended_within(Duration::from_secs(5)), "{json}");
    assert!(pair.v6.ended_within(Duration::from_secs(5)), "{json}");
}

#[test]
fn info_by_port_describes_every_holder() {
    let pair = Pair::start();
    let (json, code) = run_json(&["info", &pair.target(), "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(pids_in(&json["processes"]), pair.pids(), "{json}");
}

#[test]
fn why_traces_every_holder() {
    let pair = Pair::start();
    let (json, code) = run_json(&["why", &pair.target(), "--json"]);
    assert_eq!(code, 0, "{json}");
    let results = json["results"].as_array().unwrap();
    let mut traced: Vec<u64> = results
        .iter()
        .map(|r| r["process"]["pid"].as_u64().unwrap())
        .collect();
    traced.sort_unstable();
    assert_eq!(traced, pair.pids(), "{json}");
    for result in results {
        assert_eq!(result["port"], pair.v4.port, "{json}");
    }

    let (out, code) = run_text(&["why", &pair.target()]);
    assert_eq!(code, 0, "{out}");
    for pid in pair.pids() {
        assert!(out.contains(&format!("[{pid}]")), "{pid} missing: {out}");
    }
}

#[test]
fn wait_and_tree_by_port_take_every_holder() {
    let pair = Pair::start();
    // nothing exits, so the wait times out; the report counts both holders
    let (json, code) = run_json(&["wait", &pair.target(), "--timeout", "1", "--json"]);
    assert_eq!(code, 1, "{json}");
    assert_eq!(json["initial_count"], 2, "{json}");

    let (json, code) = run_json(&["tree", &pair.target(), "--json"]);
    assert_eq!(code, 0, "{json}");
    let roots: Vec<u64> = {
        let mut pids: Vec<u64> = json["tree"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["pid"].as_u64().unwrap())
            .collect();
        pids.sort_unstable();
        pids
    };
    assert_eq!(roots, pair.pids(), "{json}");
}

#[test]
fn watch_by_port_follows_every_holder() {
    let pair = Pair::start();
    // without a terminal, watch prints one snapshot like `list`
    let output = proc_cmd().args(["watch", &pair.target()]).output().unwrap();
    let out = String::from_utf8(output.stdout).unwrap();
    for pid in pair.pids() {
        assert!(out.contains(&pid.to_string()), "{pid} missing: {out}");
    }
}
