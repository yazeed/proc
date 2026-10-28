//! The examples people and agents copy never pair `--yes` with a name.
//!
//! `--yes` is consent given in advance, so it belongs with a `:port` or a PID
//! the person has just seen. A name can match more than they think (`proc kill
//! node --yes` once reached every Helper process of an app), so an example with
//! a name is a `--dry-run`, or runs without `--yes` and asks first. This reads
//! the places the examples live: `--help`, the command headers, the README, the
//! FAQ and the command pages.

use regex::Regex;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The files whose examples are checked
fn files() -> Vec<PathBuf> {
    let mut files = vec![
        root().join("README.md"),
        root().join("src/cli.rs"),
        root().join("docs/_data/faq.yml"),
    ];
    for dir in ["src/commands", "docs/commands"] {
        for entry in std::fs::read_dir(root().join(dir)).unwrap() {
            files.push(entry.unwrap().path());
        }
    }
    files.sort();
    files
}

/// Each `proc <command> <target> ... --yes` line whose target is a name
fn names_with_yes(path: &Path) -> Vec<String> {
    let example = Regex::new(
        r"proc\s+(?:kill|stop|freeze|thaw|free|unstick)\s+([^\s-][^\s]*)[^\n]*(?:--yes|\s-y\b)",
    )
    .unwrap();
    let text = std::fs::read_to_string(path).unwrap();
    text.lines()
        .filter_map(|line| {
            let target = example.captures(line)?.get(1)?.as_str();
            let port_or_pid = target.starts_with(':') || target.chars().all(|c| c.is_ascii_digit());
            (!port_or_pid).then(|| format!("{}: {}", path.display(), line.trim()))
        })
        .collect()
}

#[test]
fn no_example_pairs_yes_with_a_name() {
    let bad: Vec<String> = files().iter().flat_map(|f| names_with_yes(f)).collect();
    assert!(
        bad.is_empty(),
        "examples with a name and --yes:\n{}",
        bad.join("\n")
    );
}

#[test]
fn the_check_catches_a_name_and_lets_ports_and_pids_through() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("examples.md");
    std::fs::write(
        &file,
        "proc kill node --yes\n\
         proc stop nginx --signal HUP -y\n\
         proc kill :3000 --yes\n\
         proc kill 1234 5678 --yes\n\
         proc kill node --dry-run\n\
         proc unstick --force --yes\n\
         proc free :3000 :8080 --yes\n",
    )
    .unwrap();
    let bad = names_with_yes(&file);
    assert_eq!(bad.len(), 2, "{bad:?}");
    assert!(bad[0].ends_with("proc kill node --yes"), "{bad:?}");
    assert!(bad[1].ends_with("-y"), "{bad:?}");
}
