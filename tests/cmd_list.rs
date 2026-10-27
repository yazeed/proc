//! `list`, `in`, `tree` and the shared `--in` filter, driven through the real binary.
//!
//! Every process these tests look for is one the test started: a copy of the
//! listener example (`examples/listener.rs`) under a name and path of its own,
//! in a fresh temp directory, so `--by <name>` and `--in <dir>` match only it.
//! Each process is killed and reaped on drop.

use assert_cmd::Command;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

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

/// `target/debug/examples/listener`, found relative to this test binary
/// (`target/debug/deps/cmd_list-<hash>`)
fn listener_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("examples");
    path.push(format!("listener{}", std::env::consts::EXE_SUFFIX));
    assert!(path.exists(), "build the example first: {}", path.display());
    path
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Run the copy, with its cwd in `dir`. On Linux a file another thread has
/// just written can be "busy" (ETXTBSY) for an instant: a fork in a parallel
/// test still holds the write handle until it execs. That clears in
/// milliseconds, so try again.
fn spawn_copy(copy: &Path, dir: &Path) -> Child {
    let mut attempts = 0;
    loop {
        let spawned = std::process::Command::new(copy)
            .current_dir(dir)
            .stdout(Stdio::piped())
            .stdin(Stdio::null())
            .spawn();
        match spawned {
            Ok(child) => return child,
            Err(e) if attempts < 100 && is_text_file_busy(&e) => {
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(e) => panic!("spawn the listener copy: {e}"),
        }
    }
}

#[cfg(unix)]
fn is_text_file_busy(e: &std::io::Error) -> bool {
    e.raw_os_error() == Some(libc::ETXTBSY)
}

#[cfg(not(unix))]
fn is_text_file_busy(_: &std::io::Error) -> bool {
    false
}

/// A listener the test owns: a copy of the listener example, run with its cwd
/// in a fresh temp directory. Killed and reaped on drop.
struct Owned {
    child: Child,
    /// A name no other process has, short enough for any OS to report whole
    name: String,
    /// Keeps the directory alive
    _tmp: tempfile::TempDir,
    /// The directory exactly as `tempfile` named it: on macOS `/var/folders/..`
    /// (reported by processes as `/private/var/..`), on Windows possibly an
    /// 8.3 short name. This is what a user would type.
    raw_dir: PathBuf,
}

impl Owned {
    fn listener() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let raw_dir = tmp.path().to_path_buf();
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let name = format!("pcl{}n{n}x", std::process::id());
        let copy = raw_dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        std::fs::copy(listener_path(), &copy).expect("copy the listener example");
        let mut child = spawn_copy(&copy, &raw_dir);
        let mut line = String::new();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let read = stdout.read_line(&mut line);
        if read.is_err() || !line.trim().starts_with("PORT ") {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the listener printed {line:?} instead of `PORT <n>`");
        }
        Owned {
            child,
            name,
            _tmp: tmp,
            raw_dir,
        }
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn raw_dir(&self) -> String {
        self.raw_dir.to_string_lossy().into_owned()
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Whether the `processes` array of a JSON answer holds this PID
fn lists(json: &Value, pid: u32) -> bool {
    json["processes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["pid"].as_u64() == Some(u64::from(pid)))
}

// --- `--in` takes the path the way a user types it ---------------------------

#[test]
fn list_in_finds_a_process_by_the_uncanonicalized_temp_path() {
    // On macOS the temp dir is `/var/folders/..` but processes report
    // `/private/var/folders/..`; on Windows it can be `C:\Users\RUNNER~1\..`
    let owned = Owned::listener();
    let (json, code) = run_json(&["list", "--in", &owned.raw_dir(), "--json"]);
    assert_eq!(code, 0);
    assert!(lists(&json, owned.pid()), "{json}");
}

#[test]
fn in_finds_a_process_by_the_uncanonicalized_temp_path() {
    let owned = Owned::listener();
    let (json, code) = run_json(&["in", &owned.raw_dir(), "--json"]);
    assert_eq!(code, 0);
    assert!(lists(&json, owned.pid()), "{json}");
}

#[test]
fn by_in_finds_a_process_by_the_uncanonicalized_temp_path() {
    let owned = Owned::listener();
    let (json, code) = run_json(&["by", &owned.name, "--in", &owned.raw_dir(), "--json"]);
    assert_eq!(code, 0);
    assert!(lists(&json, owned.pid()), "{json}");
}

#[test]
fn tree_in_finds_a_process_by_the_uncanonicalized_temp_path() {
    let owned = Owned::listener();
    let (json, code) = run_json(&["tree", &owned.name, "--in", &owned.raw_dir(), "--json"]);
    assert_eq!(code, 0);
    let roots = json["tree"].as_array().unwrap();
    assert!(
        roots
            .iter()
            .any(|n| n["pid"].as_u64() == Some(u64::from(owned.pid()))),
        "{json}"
    );
}

#[cfg(unix)]
#[test]
fn list_in_follows_a_symlink_to_the_directory() {
    let owned = Owned::listener();
    let links = tempfile::tempdir().unwrap();
    let link = links.path().join("via-link");
    std::os::unix::fs::symlink(&owned.raw_dir, &link).unwrap();
    let (json, code) = run_json(&["list", "--in", link.to_str().unwrap(), "--json"]);
    assert_eq!(code, 0);
    assert!(lists(&json, owned.pid()), "{json}");
}

#[cfg(unix)]
#[test]
fn in_follows_a_symlink_to_the_directory() {
    let owned = Owned::listener();
    let links = tempfile::tempdir().unwrap();
    let link = links.path().join("via-link");
    std::os::unix::fs::symlink(&owned.raw_dir, &link).unwrap();
    let (json, code) = run_json(&["in", link.to_str().unwrap(), "--json"]);
    assert_eq!(code, 0);
    assert!(lists(&json, owned.pid()), "{json}");
}

// --- `tree NAME` does not match the shell that launched it --------------------

#[cfg(unix)]
#[test]
fn tree_by_name_skips_the_shell_whose_command_line_holds_the_name() {
    let owned = Owned::listener();
    let proc_exe = assert_cmd::cargo::cargo_bin("proc");
    // The `exit` keeps sh alive as proc's parent (a bare `sh -c cmd` may exec
    // cmd in place); sh's own command line holds the name
    let output = std::process::Command::new("sh")
        .args(["-c", r#""$0" "$@"; exit $?"#])
        .arg(&proc_exe)
        .args(["tree", &owned.name, "--json", "--depth", "0"])
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let pids: Vec<u64> = json["tree"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["pid"].as_u64().unwrap())
        .collect();
    assert_eq!(pids, vec![u64::from(owned.pid())], "{json}");
}
