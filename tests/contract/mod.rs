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
/// - every error has a `reason`, one of its exit code's words, and nothing
///   else has one;
/// - `ruled_out_by` comes with `reason: ruled_out`, and only then.
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
    if json.get("reason").is_some() != json.get("error").is_some() {
        return Some(format!(
            "error = {}, reason = {}: every error has a reason, and only errors do",
            json["error"], json["reason"]
        ));
    }
    if let Some(reason) = json.get("reason") {
        let words: &[&str] = match code {
            1 => &["system_error"],
            2 => &["not_found", "command_line_only", "ruled_out"],
            3 => &["permission_denied"],
            _ => &["invalid_input", "needs_confirmation", "not_supported"],
        };
        if !words.iter().any(|word| reason == word) {
            return Some(format!(
                "exit {code} with reason = {reason}: not one of {words:?}"
            ));
        }
    }
    if json.get("ruled_out_by").is_some() != (json["reason"] == "ruled_out") {
        return Some(format!(
            "reason = {}, ruled_out_by = {}: they come together",
            json["reason"], json["ruled_out_by"]
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
