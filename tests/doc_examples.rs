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
        root().join("CHANGELOG.md"),
        root().join("src/cli.rs"),
        root().join("docs/_data/faq.yml"),
        root().join("docs/skill.md"),
    ];
    for dir in ["src/commands", "docs/commands", "docs/_posts"] {
        for entry in std::fs::read_dir(root().join(dir)).unwrap() {
            files.push(entry.unwrap().path());
        }
    }
    files.sort();
    files
}

/// Flags that take the next word as their value
const TAKES_VALUE: [&str; 8] = [
    "--in",
    "-i",
    "--by",
    "-b",
    "--signal",
    "--timeout",
    "--wait",
    "-t",
];

/// The targets of an example `proc <command> ...`: the words that are neither
/// flags nor flag values, up to a comment, pipe or end of the line. `None`
/// when the line has no such example or no `--yes`.
fn targets_with_yes(line: &str) -> Option<Vec<String>> {
    let start = Regex::new(r"proc\s+(?:kill|stop|freeze|thaw|free|unstick)\s+")
        .unwrap()
        .find(line)?
        .end();
    // a comment, a pipe, a redirect, the end of a code span, or two spaces
    // (the description column of `--help`) ends the example
    let rest = line[start..]
        .split(['#', '|', '`', '>'])
        .next()
        .unwrap()
        .split("  ")
        .next()
        .unwrap();
    let mut words = rest.split_whitespace();
    let (mut targets, mut yes) = (Vec::new(), false);
    while let Some(word) = words.next() {
        if word == "--yes" || word == "-y" {
            yes = true;
        } else if TAKES_VALUE.contains(&word) {
            words.next();
        } else if !word.starts_with('-') {
            // a comma list is several targets
            targets.extend(
                word.split(',')
                    .filter(|t| !t.is_empty())
                    .map(|t| t.trim_matches(['.', ';', ')', '"', '\'']).to_string()),
            );
        }
    }
    yes.then_some(targets)
}

/// Each example line that pairs `--yes` with a target that is a name
fn names_with_yes(path: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(path).unwrap();
    text.lines()
        .filter(|line| {
            targets_with_yes(line).is_some_and(|targets| {
                targets
                    .iter()
                    .any(|t| !(t.starts_with(':') || t.chars().all(|c| c.is_ascii_digit())))
            })
        })
        .map(|line| format!("{}: {}", path.display(), line.trim()))
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
         proc kill --cmd serve --yes\n\
         proc kill --in . node --yes\n\
         proc kill --in . 1234 --yes\n\
         proc kill :3000 --by node --yes\n\
         proc free :3000 :8080 --yes\n",
    )
    .unwrap();
    let bad = names_with_yes(&file);
    assert_eq!(bad.len(), 4, "{bad:?}");
    assert!(bad[0].ends_with("proc kill node --yes"), "{bad:?}");
    assert!(bad[1].ends_with("-y"), "{bad:?}");
    assert!(bad[2].ends_with("proc kill --cmd serve --yes"), "{bad:?}");
    assert!(bad[3].ends_with("proc kill --in . node --yes"), "{bad:?}");
}
