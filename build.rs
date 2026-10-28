//! Sets `PROC_VERSION`, the version `proc --version` prints.
//!
//! A release tag build, or any build without git (a crates.io package, a
//! Docker context without `.git`), gets the plain Cargo version. Any other
//! build in a git checkout names its branch, commit and distance from the last
//! release tag. The formatting is in `src/version.rs`, shared with the crate
//! and tested there; this file only runs git. A failure to run git is never a
//! build failure: it gives the plain version.

// The `cargo:` lines go to cargo, not to a user: this is not proc's output, so
// the print macros that `clippy.toml` bans in proc itself are fine here.
#![allow(clippy::disallowed_macros)]

#[path = "src/version.rs"]
#[allow(dead_code)]
mod version;

use std::path::PathBuf;
use std::process::Command;

/// Run git in `dir` and return trimmed stdout, or `None` if it fails.
fn git(dir: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn gather(dir: &str) -> Option<version::GitFacts> {
    // Only a checkout of this package: a crate unpacked inside some other
    // repository must not report that repository's commit.
    let top = PathBuf::from(git(dir, &["rev-parse", "--show-toplevel"])?);
    if top.canonicalize().ok()? != PathBuf::from(dir).canonicalize().ok()? {
        return None;
    }
    let commit = git(dir, &["rev-parse", "--short", "HEAD"])?;
    let branch = git(dir, &["symbolic-ref", "--short", "-q", "HEAD"]);
    let describe = git(dir, &["describe", "--tags", "--long", "--match", "v*"]);
    let modified = git(dir, &["status", "--porcelain", "--untracked-files=no"])
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    // Re-run when HEAD moves, a branch ref moves, or the index changes.
    for path in ["HEAD", "index", "packed-refs"] {
        if let Some(p) = git(dir, &["rev-parse", "--git-path", path]) {
            println!("cargo:rerun-if-changed={p}");
        }
    }
    if let Some(reference) = git(dir, &["symbolic-ref", "-q", "HEAD"]) {
        if let Some(p) = git(dir, &["rev-parse", "--git-path", &reference]) {
            println!("cargo:rerun-if-changed={p}");
        }
    }
    Some(version::GitFacts {
        branch,
        commit,
        describe,
        modified,
    })
}

fn main() {
    let package = std::env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    // Uncommitted edits to tracked files are noticed when sources change.
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=GITHUB_REF_TYPE");
    println!("cargo:rerun-if-env-changed=GITHUB_REF_NAME");
    // A GitHub Actions build of this version's release tag is a release, whatever
    // its shallow checkout lets `git describe` see
    let release_tag = std::env::var("GITHUB_REF_TYPE").as_deref() == Ok("tag")
        && std::env::var("GITHUB_REF_NAME").ok() == Some(format!("v{package}"));
    let facts = if release_tag { None } else { gather(&dir) };
    println!(
        "cargo:rustc-env=PROC_VERSION={}",
        version::format_version(&package, facts.as_ref())
    );
}
