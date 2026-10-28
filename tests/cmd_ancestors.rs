//! A name target never finds the shell that ran proc.
//!
//! `sh -c "proc by <name>"` has the name in its own command line, and a shell
//! started under that name has it as its name too, so a lookup that did not
//! skip proc's ancestors would list the shell, and `kill` would take it
//! down. Unix only: the test needs a shell it can start under a name of its
//! own.
#![cfg(unix)]

use assert_cmd::cargo::CommandCargoExt;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A name no other process has, for this call alone (the tests run at the same
/// time). Ends in `x`, so no name is the start of another.
fn unique_name() -> String {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("pca{}n{n}x", std::process::id())
}

/// Run `proc <args>` from inside a shell that is called `name` (its first
/// argument, so it matches the name exactly) and has `name` in its command
/// line, working in `dir`. Returns what proc printed and its exit code.
fn from_inside_a_shell(name: &str, dir: &std::path::Path, args: &[&str]) -> (String, i32) {
    let proc_path = Command::cargo_bin("proc").unwrap();
    let script = format!(
        "'{}' {}; code=$?; exit $code",
        proc_path.get_program().to_string_lossy(),
        args.join(" ")
    );
    let output = Command::new("sh")
        .arg0(name)
        .arg("-c")
        .arg(script)
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    (
        String::from_utf8(output.stdout).unwrap(),
        output.status.code().unwrap(),
    )
}

#[test]
fn the_shell_that_ran_proc_is_never_listed() {
    let dir = tempfile::tempdir().unwrap();
    for cmd in [false, true] {
        let name = unique_name();
        let flag: &[&str] = if cmd { &["--cmd"] } else { &[] };
        let with_flag = |args: &[&str]| -> Vec<String> {
            args.iter()
                .map(|a| a.to_string())
                .chain(flag.iter().map(|a| a.to_string()))
                .collect()
        };
        let run = |args: &[&str]| {
            let args = with_flag(args);
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            from_inside_a_shell(&name, dir.path(), &args)
        };

        // nothing but the shell and proc have the name, so nothing is found:
        // exit 2 and no process in the JSON
        for args in [
            vec!["by", &name, "--json"],
            vec!["kill", &name, "--dry-run", "--json"],
            vec!["in", ".", "--by", &name, "--json"],
        ] {
            let (out, code) = run(&args);
            assert_eq!(code, 2, "{args:?} (--cmd: {cmd}): {out}");
            assert!(!out.contains("\"pid\""), "{args:?} (--cmd: {cmd}): {out}");
        }

        // watch prints a snapshot without a terminal
        let (out, code) = run(&["watch", &name]);
        assert_eq!(code, 0, "watch (--cmd: {cmd}): {out}");
        assert!(
            out.contains("No processes found"),
            "watch (--cmd: {cmd}): {out}"
        );
    }
}
