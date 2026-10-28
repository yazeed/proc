//! The deprecation notice for comma-separated targets, through the real binary.
//!
//! `proc kill :3000,:8080` still works, but it prints one `note:` line on
//! stderr in human output, naming the form to use. `--json` output is never
//! touched. Every command here is a dry run or a read, aimed at PIDs above
//! every OS maximum, so nothing on the machine can be changed.

use assert_cmd::Command;

/// PIDs no OS hands out
const A: &str = "99999999";
const B: &str = "99999998";
const C: &str = "99999997";

fn run(args: &[&str]) -> (String, String) {
    let output = Command::cargo_bin("proc")
        .unwrap()
        .env("NO_COLOR", "1")
        .args(args)
        .output()
        .unwrap();
    (
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

/// The `note:` lines of a stderr text
fn notes(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|line| line.starts_with("note:"))
        .collect()
}

fn notice_for(command: &str, example: &str) -> String {
    format!(
        "note: comma-separated targets are deprecated and will be removed in a future \
         major release; pass them as separate arguments: proc {command} {example}"
    )
}

#[test]
fn a_comma_list_prints_the_notice_with_the_users_own_targets() {
    let list = format!("{A},{B}");
    let (stdout, stderr) = run(&["kill", &list, "--dry-run"]);
    assert_eq!(
        notes(&stderr),
        vec![notice_for("kill", &format!("{A} {B}"))]
    );
    assert!(!stdout.contains("deprecated"), "stdout: {stdout}");
    assert!(
        !stderr.contains('\x1b'),
        "no colour off a terminal: {stderr:?}"
    );
}

#[test]
fn the_example_keeps_the_other_targets_and_the_real_command_name() {
    // `k` is `kill`; the separate argument and the mixed-in list both show
    let list = format!("{B},{C}");
    let (_, stderr) = run(&["k", A, &list, "--dry-run"]);
    assert_eq!(
        notes(&stderr),
        vec![notice_for("kill", &format!("{A} {B} {C}"))]
    );
}

#[test]
fn it_prints_once_however_many_values_have_commas() {
    let first = format!("{A},{B}");
    let second = format!("{C},{A},{B}");
    let (_, stderr) = run(&["kill", &first, &second, "--dry-run"]);
    assert_eq!(notes(&stderr).len(), 1, "{stderr}");
    assert_eq!(stderr.matches("deprecated").count(), 1, "{stderr}");
}

#[test]
fn separate_arguments_print_no_notice() {
    let (_, stderr) = run(&["kill", A, B, "--dry-run"]);
    assert!(!stderr.contains("deprecated"), "{stderr}");
    assert!(notes(&stderr).is_empty(), "{stderr}");
}

#[test]
fn json_output_is_identical_with_commas_on_stdout_and_stderr() {
    // a list of names that match nothing: the same document either way
    let with_commas = run(&["list", "nosuch1,nosuch2", "--json"]);
    let separate = run(&["list", "nosuch1", "nosuch2", "--json"]);
    assert_eq!(with_commas, separate);
    assert!(with_commas.1.is_empty(), "stderr: {}", with_commas.1);
    assert_eq!(run(&["list", "nosuch1,nosuch2", "-j"]), separate);

    // an error is JSON on stdout, and stderr stays empty: the message quotes
    // the targets as typed, which is how it was before the notice
    let list = format!("{A},{B}");
    let (stdout, stderr) = run(&["kill", &list, "--dry-run", "--json"]);
    assert!(stderr.is_empty(), "stderr: {stderr}");
    assert!(
        stdout.contains(&format!("No process found matching '{A},{B}'")),
        "{stdout}"
    );
}

#[test]
fn a_trailing_or_empty_piece_is_dropped_as_before_and_still_noticed() {
    let (_, stderr) = run(&["kill", &format!("{A},"), "--dry-run"]);
    assert_eq!(notes(&stderr), vec![notice_for("kill", A)]);
    // nothing but commas leaves no target, and the notice does not invent one
    let (_, stderr) = run(&["list", ",", "--json"]);
    assert!(stderr.is_empty(), "{stderr}");
}

#[test]
fn a_name_that_needs_quoting_is_quoted_in_the_example() {
    let (_, stderr) = run(&["kill", "my app,it's", "--dry-run"]);
    assert_eq!(
        notes(&stderr),
        vec![notice_for("kill", r"'my app' 'it'\''s'")]
    );
}

#[test]
fn commas_in_flag_values_and_single_value_commands_do_not_count() {
    for args in [
        vec!["kill", A, "--by", "x,y", "--dry-run"],
        vec!["kill", A, "--in=/no/such,dir", "--dry-run"],
        vec!["list", "--by", "x,y"],
        vec!["by", "x,y"],
        vec!["in", "/no/such,dir"],
        vec!["for", "/no/such,file"],
        vec!["tree", "x,y"],
        vec!["ports", "--by", "x,y"],
    ] {
        let (_, stderr) = run(&args);
        assert!(!stderr.contains("deprecated"), "{args:?}: {stderr}");
    }
}

#[test]
fn every_command_that_splits_targets_gives_the_notice() {
    for command in [
        "on", "info", "list", "kill", "stop", "unstick", "freeze", "thaw", "why", "free", "wait",
    ] {
        // `free` takes ports, the rest take PIDs; all of them are made up
        let (first, second) = if command == "free" {
            (":65530", ":65531")
        } else {
            (A, B)
        };
        let list = format!("{first},{second}");
        let mut args = vec![command, &list];
        match command {
            "kill" | "stop" | "unstick" | "freeze" | "thaw" | "free" => args.push("--dry-run"),
            "wait" => args.extend(["--timeout", "1"]),
            _ => {}
        }
        let (_, stderr) = run(&args);
        assert_eq!(
            notes(&stderr),
            vec![notice_for(command, &format!("{first} {second}"))],
            "{command}"
        );
    }
}
