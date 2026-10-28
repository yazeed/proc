//! The version string `proc --version` prints.
//!
//! `main` keeps the last released version number until the next release, so a
//! build from `main` would print the same `1.14.5` as the real 1.14.5. This
//! module names the commit for any build that is not the release tag itself.
//!
//! `build.rs` compiles this file too (through `#[path]`): it gathers the git
//! facts, calls [`format_version`] and hands the result to the crate as
//! `PROC_VERSION`. The logic lives here, in the crate, so it is tested and
//! counted by the coverage gate; `build.rs` only runs git.

/// What git says about the checkout a build is made from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitFacts {
    /// Current branch, or `None` when HEAD is detached.
    pub branch: Option<String>,
    /// Abbreviated hash of HEAD.
    pub commit: String,
    /// Output of `git describe --tags --long --match 'v*'`, if a `v*` tag is reachable.
    pub describe: Option<String>,
    /// Whether tracked files have uncommitted changes.
    pub modified: bool,
}

/// Split `git describe --long` output (`v2.0.0-3-g5e94647`) into the tag and
/// the number of commits after it.
fn parse_describe(describe: &str) -> Option<(&str, u32)> {
    let (rest, _hash) = describe.rsplit_once('-')?;
    let (tag, count) = rest.rsplit_once('-')?;
    Some((tag, count.parse().ok()?))
}

/// The version to show. `package_version` is Cargo's version; `facts` is
/// `None` outside a git checkout (crates.io packages, Docker contexts without
/// `.git`), which gives the plain version.
pub fn format_version(package_version: &str, facts: Option<&GitFacts>) -> String {
    let Some(facts) = facts else {
        return package_version.to_string();
    };
    let since = facts.describe.as_deref().and_then(parse_describe);
    if !facts.modified && since == Some((format!("v{package_version}").as_str(), 0)) {
        return package_version.to_string();
    }
    let branch = facts.branch.as_deref().unwrap_or("detached");
    let mut details = format!("{branch}@{}", facts.commit);
    if let Some((tag, count)) = since {
        let noun = if count == 1 { "commit" } else { "commits" };
        details.push_str(&format!(", {count} {noun} after {tag}"));
    }
    if facts.modified {
        details.push_str(", modified");
    }
    format!("{package_version}-dev ({details})")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(describe: Option<&str>, modified: bool) -> GitFacts {
        GitFacts {
            branch: Some("main".into()),
            commit: "5e94647".into(),
            describe: describe.map(String::from),
            modified,
        }
    }

    #[test]
    fn no_git_is_the_plain_version() {
        assert_eq!(format_version("2.0.0", None), "2.0.0");
    }

    #[test]
    fn the_clean_release_tag_is_the_plain_version() {
        let f = facts(Some("v2.0.0-0-g5e94647"), false);
        assert_eq!(format_version("2.0.0", Some(&f)), "2.0.0");
    }

    #[test]
    fn commits_after_the_tag_are_counted() {
        let f = facts(Some("v2.0.0-3-g5e94647"), false);
        assert_eq!(
            format_version("2.0.0", Some(&f)),
            "2.0.0-dev (main@5e94647, 3 commits after v2.0.0)"
        );
        let f = facts(Some("v2.0.0-1-g5e94647"), false);
        assert_eq!(
            format_version("2.0.0", Some(&f)),
            "2.0.0-dev (main@5e94647, 1 commit after v2.0.0)"
        );
    }

    #[test]
    fn main_before_the_release_counts_from_the_last_tag() {
        let f = facts(Some("v1.14.5-12-g5e94647"), false);
        assert_eq!(
            format_version("1.14.5", Some(&f)),
            "1.14.5-dev (main@5e94647, 12 commits after v1.14.5)"
        );
    }

    #[test]
    fn a_modified_tree_says_so_even_on_the_tag() {
        let f = facts(Some("v2.0.0-0-g5e94647"), true);
        assert_eq!(
            format_version("2.0.0", Some(&f)),
            "2.0.0-dev (main@5e94647, 0 commits after v2.0.0, modified)"
        );
        let f = facts(Some("v2.0.0-3-g5e94647"), true);
        assert_eq!(
            format_version("2.0.0", Some(&f)),
            "2.0.0-dev (main@5e94647, 3 commits after v2.0.0, modified)"
        );
    }

    #[test]
    fn a_detached_head_is_named() {
        let mut f = facts(Some("v2.0.0-3-g5e94647"), false);
        f.branch = None;
        assert_eq!(
            format_version("2.0.0", Some(&f)),
            "2.0.0-dev (detached@5e94647, 3 commits after v2.0.0)"
        );
    }

    #[test]
    fn a_tag_for_another_version_is_not_this_release() {
        let f = facts(Some("v1.14.5-0-g5e94647"), false);
        assert_eq!(
            format_version("2.0.0", Some(&f)),
            "2.0.0-dev (main@5e94647, 0 commits after v1.14.5)"
        );
    }

    #[test]
    fn no_tag_reachable_leaves_out_the_count() {
        let f = facts(None, false);
        assert_eq!(
            format_version("2.0.0", Some(&f)),
            "2.0.0-dev (main@5e94647)"
        );
        let f = facts(Some("not-a-describe"), false);
        assert_eq!(
            format_version("2.0.0", Some(&f)),
            "2.0.0-dev (main@5e94647)"
        );
        let f = facts(Some("nodash"), true);
        assert_eq!(
            format_version("2.0.0", Some(&f)),
            "2.0.0-dev (main@5e94647, modified)"
        );
    }
}
