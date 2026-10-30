//! The `--json` contract every document keeps, checked wherever a test reads
//! one (skills/proc-cli/reference.md, "Exit codes"; PHILOSOPHY.md, "One
//! Contract"). The document being one JSON value is checked by the parse
//! itself: serde refuses anything after the first value.

// Each test binary uses one of the two
#![allow(dead_code)]

use serde_json::Value;

/// What is wrong with a document `proc` printed before exiting with `code`,
/// if anything:
/// - the exit code is one of proc's, 0 to 4;
/// - `action` names the command;
/// - `success` is true exactly when the exit code is 0;
/// - `exit_code`, which every error has, is the exit code;
/// - `reason` comes with exit 4, and only then.
pub fn violation(json: &Value, code: i32) -> Option<String> {
    if !(0..=4).contains(&code) {
        return Some(format!("exit {code} is not one of proc's codes, 0 to 4"));
    }
    if !json["action"].is_string() {
        return Some("no `action`".to_string());
    }
    if json["success"] != (code == 0) {
        return Some(format!("exit {code} but success = {}", json["success"]));
    }
    if json.get("error").is_some() && json.get("exit_code").is_none() {
        return Some("an `error` without `exit_code`".to_string());
    }
    if json.get("exit_code").is_some_and(|field| *field != code) {
        return Some(format!("exit {code} but exit_code = {}", json["exit_code"]));
    }
    if json.get("reason").is_some() != (code == 4) {
        return Some(format!(
            "exit {code} with reason = {}: a reason comes with exit 4 only",
            json["reason"]
        ));
    }
    None
}

/// Panic unless the document keeps the contract
pub fn check(json: &Value, code: i32, args: &[&str]) {
    if let Some(wrong) = violation(json, code) {
        panic!("proc {args:?} broke the JSON contract: {wrong}\n{json:#}");
    }
}
