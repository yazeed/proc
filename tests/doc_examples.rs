//! The examples people and agents copy never pair `--yes` with a name.
//!
//! `--yes` is consent given in advance, so it belongs with a `:port` or a PID
//! the person has just seen. A name can match more than they think (`proc kill
//! node --yes` once reached every Helper process of an app), so an example with
//! a name is a `--dry-run`, or runs without `--yes` and asks first. This reads
//! the places the examples live: `--help`, the command headers, the README, the
//! FAQ and the command pages, and the agent skill (`skills/proc-cli/*.md`),
//! which an agent copies from without a human in between. The skill also never
//! shows a comma-separated list of targets, which is deprecated, except on a
//! line that says so.

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
    for dir in [
        "src/commands",
        "docs/commands",
        "docs/_posts",
        "skills/proc-cli",
    ] {
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
    // a `<pid>` placeholder is a word, not a redirect: set it apart before the
    // `>` below ends the example
    let line = Regex::new(r"<([a-z0-9]+)>")
        .unwrap()
        .replace_all(line, "\u{ab}$1\u{bb}");
    let start = Regex::new(r"proc\s+(?:kill|stop|freeze|thaw|free|unstick)\s+")
        .unwrap()
        .find(&line)?
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
                targets.iter().any(|t| {
                    // a port, a PID, or the `<pid>` placeholder for one
                    !(t.starts_with(':')
                        || t.chars().all(|c| c.is_ascii_digit())
                        || t == "\u{ab}pid\u{bb}")
                })
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
         proc free :3000 :8080 --yes\n\
         proc stop <pid> <pid> --in=<dir> --by <name> --yes\n\
         proc free :<port> --yes\n\
         proc stop <name> --in=<dir> --yes\n\
         proc stop <pid> --yes > out\n",
    )
    .unwrap();
    let bad = names_with_yes(&file);
    assert_eq!(bad.len(), 5, "{bad:?}");
    assert!(bad[0].ends_with("proc kill node --yes"), "{bad:?}");
    assert!(bad[1].ends_with("-y"), "{bad:?}");
    assert!(bad[2].ends_with("proc kill --cmd serve --yes"), "{bad:?}");
    assert!(bad[3].ends_with("proc kill --in . node --yes"), "{bad:?}");
    // a `<name>` placeholder is a name; `<pid>` and `:<port>` are not
    assert!(
        bad[4].ends_with("proc stop <name> --in=<dir> --yes"),
        "{bad:?}"
    );
}

/// The skill files, which are written for agents
fn skill_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(root().join("skills/proc-cli"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    files.sort();
    files
}

/// Each example line with a command and a comma-separated list of targets,
/// unless the line says commas are deprecated
fn comma_lists(path: &Path) -> Vec<String> {
    let example = Regex::new(
        r"proc\s+(?:on|for|by|in|list|info|ports|why|tree|watch|wait|kill|stop|freeze|thaw|free|unstick)\s+([^`|#>]*)",
    )
    .unwrap();
    let text = std::fs::read_to_string(path).unwrap();
    text.lines()
        .filter(|line| !line.contains("deprecated"))
        .filter(|line| {
            example.captures_iter(line).any(|c| {
                // a comma inside a word, not the one that ends a sentence
                c[1].split_whitespace()
                    .any(|word| !word.starts_with('-') && word.trim_end_matches(',').contains(','))
            })
        })
        .map(|line| format!("{}: {}", path.display(), line.trim()))
        .collect()
}

#[test]
fn the_skill_shows_no_comma_separated_targets() {
    let bad: Vec<String> = skill_files().iter().flat_map(|f| comma_lists(f)).collect();
    assert!(
        bad.is_empty(),
        "skill examples with a comma list:\n{}",
        bad.join("\n")
    );
}

#[test]
fn the_skill_never_shows_yes_with_orphans_or_stuck() {
    // they take no target, so `--yes` there acts on a list nobody has seen
    let pair = Regex::new(r"proc\s+(?:orphans|stuck)\b[^`|#]*(?:--yes|\s-y\b)").unwrap();
    let bad: Vec<String> = skill_files()
        .iter()
        .flat_map(|path| {
            std::fs::read_to_string(path)
                .unwrap()
                .lines()
                .filter(|line| pair.is_match(line))
                .map(|line| format!("{}: {}", path.display(), line.trim()))
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(
        bad.is_empty(),
        "orphans or stuck with --yes:\n{}",
        bad.join("\n")
    );
}

#[test]
fn the_comma_check_catches_a_list_and_lets_a_deprecation_note_through() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("skill.md");
    std::fs::write(
        &file,
        "proc kill 1234,5678 --dry-run\n\
         proc list 'git commit,pre-commit'\n\
         proc on :3000,:8080 --json\n\
         Commas (proc on :3000,:8080) still work but are deprecated.\n\
         proc kill 1234 5678 --dry-run\n\
         proc ports --range 3000-3999 --json\n",
    )
    .unwrap();
    let bad = comma_lists(&file);
    assert_eq!(bad.len(), 3, "{bad:?}");
    assert!(bad[0].ends_with("proc kill 1234,5678 --dry-run"), "{bad:?}");
    assert!(bad[2].ends_with("proc on :3000,:8080 --json"), "{bad:?}");
}
