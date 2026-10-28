//! Parser errors name what is wrong.
//!
//! The corpus is generated from the CLI definition (`Cli::command()`): for every
//! subcommand and each of its arguments a missing required argument, an unknown
//! flag, a bad value, a missing value after a flag and too many values, each run
//! with `--json` and without. Whatever clap says, the `error` must be whole
//! (never cut after a colon) and name the argument or the value at fault, and the
//! allowed values for a value with a fixed set.
//!
//! Every case fails while parsing, so no process is looked up or touched.

use assert_cmd::Command;
use clap::{Arg, CommandFactory};
use proc_cli::cli::Cli;
use serde_json::Value;
use std::any::TypeId;

fn proc_cmd() -> Command {
    let mut cmd = Command::cargo_bin("proc").unwrap();
    cmd.env("NO_COLOR", "1");
    cmd
}

/// One generated case: `proc <args...>`, and what the message must contain
#[derive(Debug)]
struct Case {
    /// What the case checks, for the failure message
    what: String,
    args: Vec<String>,
    /// Every one of these must be in the message
    needles: Vec<String>,
    /// Whether the case may run with `--json` (the subcommand has the flag)
    json: bool,
}

/// The name a missing argument is listed under: `TARGET` for a positional, the
/// long flag for an option
fn display_name(arg: &Arg) -> String {
    if arg.is_positional() {
        arg.get_value_names()
            .and_then(|names| names.first())
            .map(|name| name.to_string())
            .unwrap_or_else(|| arg.get_id().as_str().to_uppercase())
    } else {
        format!("--{}", arg.get_long().expect("an option has a long flag"))
    }
}

fn takes_value(arg: &Arg) -> bool {
    arg.get_action().takes_values()
}

/// Whether a value is required after the flag (`--in` takes an optional one)
fn needs_value(arg: &Arg) -> bool {
    takes_value(arg) && arg.get_num_args().is_none_or(|n| n.min_values() >= 1)
}

fn allowed_values(arg: &Arg) -> Vec<String> {
    arg.get_possible_values()
        .iter()
        .filter(|value| !value.is_hide_set())
        .map(|value| value.get_name().to_string())
        .collect()
}

/// Whether the value parser is a number type
fn is_numeric(arg: &Arg) -> bool {
    let id = arg.get_value_parser().type_id();
    [
        TypeId::of::<u8>(),
        TypeId::of::<u16>(),
        TypeId::of::<u32>(),
        TypeId::of::<u64>(),
        TypeId::of::<usize>(),
        TypeId::of::<i32>(),
        TypeId::of::<i64>(),
        TypeId::of::<f32>(),
        TypeId::of::<f64>(),
    ]
    .iter()
    .any(|number| id == *number)
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| a.to_string()).collect()
}

/// Every case, generated from the definition
fn corpus() -> Vec<Case> {
    let root = Cli::command();
    let mut cases = vec![
        Case {
            what: "root: unknown flag".to_string(),
            args: strings(&["--definitely-not-a-flag"]),
            needles: strings(&["--definitely-not-a-flag"]),
            json: true,
        },
        Case {
            what: "root: unknown subcommand".to_string(),
            args: strings(&["definitely-not-a-command"]),
            needles: strings(&["definitely-not-a-command"]),
            json: true,
        },
    ];

    for sub in root.get_subcommands() {
        let name = sub.get_name().to_string();
        let args: Vec<&Arg> = sub
            .get_arguments()
            .filter(|a| !matches!(a.get_id().as_str(), "help" | "version"))
            .collect();
        let has_json = args.iter().any(|a| a.get_long() == Some("json"));
        let case = |what: String, mut rest: Vec<String>, needles: Vec<String>| {
            let mut all = vec![name.clone()];
            all.append(&mut rest);
            Case {
                what: format!("{name}: {what}"),
                args: all,
                needles,
                json: has_json,
            }
        };

        // an unknown flag
        cases.push(case(
            "unknown flag".to_string(),
            strings(&["--definitely-not-a-flag"]),
            strings(&["--definitely-not-a-flag"]),
        ));

        // the required arguments, all missing
        let required: Vec<&&Arg> = args.iter().filter(|a| a.is_required_set()).collect();
        if !required.is_empty() {
            cases.push(case(
                "missing required arguments".to_string(),
                Vec::new(),
                required.iter().map(|a| display_name(a)).collect(),
            ));
        }

        for arg in &args {
            let shown = display_name(arg);
            let allowed = allowed_values(arg);

            // a value outside the allowed set, or one the type does not parse
            if takes_value(arg) && (!allowed.is_empty() || is_numeric(arg)) {
                let bad = "bogus-value".to_string();
                let mut needles = vec![bad.clone()];
                needles.extend(allowed.clone());
                let rest = if arg.is_positional() {
                    vec![bad]
                } else {
                    vec![shown.clone(), bad]
                };
                cases.push(case(format!("bad value for {shown}"), rest, needles));
            }

            // a flag at the end of the line, its value missing
            if !arg.is_positional() && needs_value(arg) {
                cases.push(case(
                    format!("missing value for {shown}"),
                    vec![shown.clone()],
                    vec![shown.clone()],
                ));
            }

            // more values than a bounded positional takes
            if arg.is_positional() {
                let max = arg.get_num_args().map_or(1, |n| n.max_values());
                if max < 8 {
                    let valid = allowed
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "val".to_string());
                    let mut rest = vec![valid; max];
                    rest.push("extra-arg".to_string());
                    cases.push(case(
                        format!("too many values for {shown}"),
                        rest,
                        strings(&["extra-arg"]),
                    ));
                }
            }
        }
    }
    cases
}

/// What clap adds around a message that does not belong in the JSON `error`
fn assert_whole(error: &str, what: &str) {
    assert!(!error.is_empty(), "{what}: empty error");
    assert!(
        !error.ends_with(':'),
        "{what}: cut after a colon: {error:?}"
    );
    for noise in ["Usage:", "For more information", "tip:"] {
        assert!(!error.contains(noise), "{what}: {noise:?} in {error:?}");
    }
}

#[test]
fn every_parser_error_names_what_is_wrong() {
    let cases = corpus();
    // the generator found the shapes: more than a handful per subcommand
    assert!(cases.len() >= 100, "only {} cases", cases.len());

    for case in &cases {
        let what = &case.what;

        // human: clap's rendering, on stderr
        let out = proc_cmd().args(&case.args).output().unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(4), "{what}: {stderr}");
        assert!(out.stdout.is_empty(), "{what}: stdout is not empty");
        for needle in &case.needles {
            assert!(
                stderr.contains(needle),
                "{what}: {needle:?} not in {stderr}"
            );
        }

        // JSON: one document whose `error` is clap's message in one piece
        if !case.json {
            continue;
        }
        let out = proc_cmd().args(&case.args).arg("--json").output().unwrap();
        assert_eq!(out.status.code(), Some(4), "{what}");
        let stdout = String::from_utf8_lossy(&out.stdout);
        let json: Value = serde_json::from_str(&stdout)
            .unwrap_or_else(|e| panic!("{what}: not one JSON document ({e}): {stdout}"));
        assert_eq!(json["reason"], "invalid_input", "{what}: {json}");
        assert_eq!(json["exit_code"], 4, "{what}: {json}");
        assert_eq!(json["success"], false, "{what}: {json}");
        let error = json["error"].as_str().expect("error is text");
        assert_whole(error, what);
        for needle in &case.needles {
            assert!(
                error.contains(needle),
                "{what}: {needle:?} not in {error:?}"
            );
        }
    }
}

#[test]
fn the_corpus_covers_the_shapes_the_bugs_had() {
    let cases = corpus();
    for (what, needle) in [
        ("kill: missing required arguments", "TARGET"),
        ("list: bad value for --sort", "cpu"),
        ("list: bad value for --status", "zombie"),
        ("list: missing value for --sort", "--sort"),
    ] {
        assert!(
            cases
                .iter()
                .any(|c| c.what == what && c.needles.iter().any(|n| n == needle)),
            "no case {what:?} with {needle:?}"
        );
    }
}

fn json_error(args: &[&str]) -> String {
    let out = proc_cmd().args(args).output().unwrap();
    assert_eq!(out.status.code(), Some(4));
    let json: Value = serde_json::from_slice(&out.stdout).unwrap();
    json["error"].as_str().unwrap().to_string()
}

#[test]
fn the_json_error_keeps_the_missing_argument_and_the_allowed_values() {
    assert_eq!(
        json_error(&["kill", "-j"]),
        "the following required arguments were not provided: <TARGET>..."
    );
    assert_eq!(
        json_error(&["list", "--sort", "bogus", "-j"]),
        "invalid value 'bogus' for '--sort <SORT>' [possible values: cpu, mem, pid, name]"
    );
    assert_eq!(
        json_error(&["list", "--status", "bogus", "-j"]),
        "invalid value 'bogus' for '--status <STATUS>' \
         [possible values: running, sleeping, stopped, zombie]"
    );
    // the "did you mean" suggestion stays
    assert_eq!(
        json_error(&["kill", "123", "--jsn", "-j"]),
        "unexpected argument '--jsn' found\n  Try: did you mean '--json'?"
    );
}

const HINT_BY: &str =
    "kill needs at least one target; --by filters the targets, it doesn't select them";
const HINT_IN: &str =
    "kill needs at least one target; --in filters the targets, it doesn't select them";

#[test]
fn a_filter_without_a_target_says_the_filter_does_not_select() {
    for (args, hint) in [
        (&["kill", "--by", "wispworker"][..], HINT_BY),
        (&["kill", "--by=wispworker"], HINT_BY),
        (&["kill", "-b", "wispworker"], HINT_BY),
        (&["kill", "-yb", "wispworker"], HINT_BY),
        (&["kill", "--in"], HINT_IN),
        (&["kill", "--in=."], HINT_IN),
        (&["kill", "-i"], HINT_IN),
        // an alias is named by its command; a value-taking short flag before it
        (&["k", "--in", "."], HINT_IN),
    ] {
        // human
        let out = proc_cmd().args(args).output().unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(4), "{args:?}");
        assert!(stderr.contains(hint), "{args:?}: {stderr}");
        assert!(stderr.contains("Usage:"), "{args:?}: {stderr}");
        // JSON
        let mut json_args = args.to_vec();
        json_args.push("--json");
        let error = json_error(&json_args);
        assert!(
            error.starts_with("the following required arguments were not provided: <TARGET>..."),
            "{args:?}: {error}"
        );
        assert!(error.ends_with(hint), "{args:?}: {error}");
    }

    // a short flag that takes a value does not hide the filter after it
    let error = json_error(&["wait", "-t", "5", "-b", "x", "-j"]);
    assert!(
        error.ends_with(
            "wait needs at least one target; --by filters the targets, it doesn't select them"
        ),
        "{error}"
    );
}

#[test]
fn no_hint_where_no_filter_was_given_or_no_target_is_missing() {
    // no filter
    assert!(!json_error(&["kill", "-j"]).contains("filters"));
    // a missing argument that is not the target
    assert!(!json_error(&["in", "--by", "x", "-j"]).contains("filters the targets"));
    // another error with a filter on the line
    assert!(!json_error(&["kill", "--by", "x", "--bogus", "-j"]).contains("filters the targets"));
}

#[cfg(unix)]
#[test]
fn a_non_utf8_argument_is_invalid_input_not_a_panic() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let odd = OsStr::from_bytes(b"\xff");
    let odd_path = OsStr::from_bytes(b"/tmp/\xff");
    let odd_in = OsStr::from_bytes(b"--in=\xff");
    for args in [
        vec![OsStr::new("kill"), odd],
        vec![OsStr::new("list"), odd],
        vec![OsStr::new("in"), odd_path],
        vec![OsStr::new("list"), OsStr::new("--in"), odd],
        vec![OsStr::new("list"), odd_in],
        vec![OsStr::new("list"), OsStr::new("--by"), odd],
    ] {
        // human
        let out = proc_cmd().args(&args).output().unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(4), "{args:?}: {stderr}");
        assert!(out.stdout.is_empty(), "{args:?}");
        assert!(stderr.contains("is not valid UTF-8"), "{args:?}: {stderr}");
        assert!(stderr.contains('\u{fffd}'), "{args:?}: {stderr}");
        assert!(!stderr.contains("panicked"), "{args:?}: {stderr}");

        // JSON
        let out = proc_cmd().args(&args).arg("--json").output().unwrap();
        assert_eq!(out.status.code(), Some(4), "{args:?}");
        let json: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(json["reason"], "invalid_input", "{json}");
        assert_eq!(json["action"], args[0].to_str().unwrap(), "{json}");
        let error = json["error"].as_str().unwrap();
        assert!(error.contains("is not valid UTF-8"), "{json}");
        assert!(error.contains('\u{fffd}'), "{json}");
    }
}
