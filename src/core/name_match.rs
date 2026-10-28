//! How a name target (and a `--by` filter) is compared with a process.
//!
//! By default a name matches a process the way `killall` and `pkill` do: the
//! whole name, equal ignoring case. `--cmd` matches the way `pkill -f` does:
//! the text anywhere in the name or the full command line. The default is the
//! safe one: `proc kill node` must not reach "Claude Helper" or an editor
//! whose command line happens to mention node.

/// Which comparison a name target uses
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NameMatch {
    /// The text equals the process name, the file name of its executable, or
    /// the file name of its first argument, ignoring case (the default)
    #[default]
    Exact,
    /// The text is a substring of the process name or the full command line,
    /// ignoring case (`--cmd`)
    Command,
}

impl NameMatch {
    /// The comparison a command uses, given whether `--cmd` was passed
    pub fn from_flag(cmd: bool) -> Self {
        if cmd {
            NameMatch::Command
        } else {
            NameMatch::Exact
        }
    }
}

/// Everything about a process a name can be compared with
#[derive(Debug, Clone, Copy, Default)]
pub struct Names<'a> {
    /// The process name the OS reports (on Linux cut to 15 characters)
    pub name: &'a str,
    /// Path of the executable, when the OS would tell
    pub exe: Option<&'a str>,
    /// The first word of the command line, when the OS would tell
    pub argv0: Option<&'a str>,
    /// The whole command line, when the OS would tell
    pub command: Option<&'a str>,
}

/// How a name relates to a process
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// It matches under the comparison in use
    Match,
    /// It does not, but the text is in the command line, so `--cmd` would match
    CommandLineOnly,
    /// It matches neither way
    No,
}

/// The last component of a path. A backslash separates components only on
/// Windows: on Unix it can be part of a file name.
fn base_name(path: &str, windows: bool) -> &str {
    path.rsplit(|c| c == '/' || (windows && c == '\\'))
        .next()
        .unwrap_or(path)
}

/// `name` without a trailing `.exe`, in any case
fn without_exe(name: &str) -> &str {
    match name.len().checked_sub(4).and_then(|at| name.get(at..)) {
        Some(tail) if tail.eq_ignore_ascii_case(".exe") => &name[..name.len() - 4],
        _ => name,
    }
}

/// Whether two names are the same ignoring case; on Windows a trailing `.exe`
/// is optional on either side
fn same_name(a: &str, b: &str, windows: bool) -> bool {
    let (a, b) = if windows {
        (without_exe(a), without_exe(b))
    } else {
        (a, b)
    };
    a.to_lowercase() == b.to_lowercase()
}

/// Whether `pattern` is the process name, the file name of its executable or
/// the file name of its first argument
///
/// The two extra names are there because Linux cuts the process name to 15
/// characters, so a longer executable name is only found through them.
fn is_exact(pattern: &str, names: &Names<'_>, windows: bool) -> bool {
    same_name(pattern, names.name, windows)
        || names
            .exe
            .is_some_and(|exe| same_name(pattern, base_name(exe, windows), windows))
        || names
            .argv0
            .is_some_and(|argv0| same_name(pattern, base_name(argv0, windows), windows))
}

/// Whether `pattern` is a substring of the process name or the command line
fn in_command_line(pattern: &str, names: &Names<'_>) -> bool {
    let pattern = pattern.to_lowercase();
    names.name.to_lowercase().contains(&pattern)
        || names
            .command
            .is_some_and(|command| command.to_lowercase().contains(&pattern))
}

/// How `pattern` relates to a process under `mode`; `windows` says whether
/// names follow Windows rules (an optional `.exe`, `\` in paths)
pub fn classify_with(pattern: &str, names: &Names<'_>, mode: NameMatch, windows: bool) -> Hit {
    match mode {
        // nothing is named "": it is not a name, and not worth counting
        // command lines for, since every one contains it
        NameMatch::Exact if pattern.is_empty() => Hit::No,
        NameMatch::Exact if is_exact(pattern, names, windows) => Hit::Match,
        NameMatch::Exact if in_command_line(pattern, names) => Hit::CommandLineOnly,
        NameMatch::Command if in_command_line(pattern, names) => Hit::Match,
        _ => Hit::No,
    }
}

/// [`classify_with`] under this OS's rules
pub fn classify(pattern: &str, names: &Names<'_>, mode: NameMatch) -> Hit {
    classify_with(pattern, names, mode, cfg!(windows))
}

/// Whether `pattern` matches a process under `mode`, by this OS's rules
pub fn matches_name(pattern: &str, names: &Names<'_>, mode: NameMatch) -> bool {
    classify(pattern, names, mode) == Hit::Match
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXACT: NameMatch = NameMatch::Exact;
    const CMD: NameMatch = NameMatch::Command;

    fn unix(pattern: &str, names: &Names<'_>, mode: NameMatch) -> Hit {
        classify_with(pattern, names, mode, false)
    }

    fn windows(pattern: &str, names: &Names<'_>, mode: NameMatch) -> Hit {
        classify_with(pattern, names, mode, true)
    }

    /// A node server as `ps` shows it: `node /srv/app/server.js --port 3000`
    fn node_server() -> Names<'static> {
        Names {
            name: "node",
            exe: Some("/usr/local/bin/node"),
            argv0: Some("node"),
            command: Some("node /srv/app/server.js --port 3000"),
        }
    }

    #[test]
    fn the_flag_picks_the_comparison() {
        assert_eq!(NameMatch::from_flag(false), NameMatch::Exact);
        assert_eq!(NameMatch::from_flag(true), NameMatch::Command);
        assert_eq!(NameMatch::default(), NameMatch::Exact);
    }

    #[test]
    fn the_whole_name_matches_and_a_part_of_it_does_not() {
        let helper = Names {
            name: "Claude Helper",
            exe: Some("/Applications/Claude.app/Contents/Frameworks/Claude Helper"),
            argv0: Some("/Applications/Claude.app/Contents/Frameworks/Claude Helper"),
            command: None,
        };
        assert_eq!(unix("Claude Helper", &helper, EXACT), Hit::Match);
        assert_eq!(unix("Claude", &helper, EXACT), Hit::CommandLineOnly);
        assert_eq!(unix("Helper", &helper, EXACT), Hit::CommandLineOnly);
        assert_eq!(unix("node", &helper, EXACT), Hit::No);
        assert_eq!(unix("node", &node_server(), EXACT), Hit::Match);
        assert_eq!(unix("nod", &node_server(), EXACT), Hit::CommandLineOnly);
    }

    #[test]
    fn an_empty_name_matches_nothing_and_hints_at_nothing() {
        // every command line contains the empty string, so by default it must
        // not count as "in the command line" for a hint to count them
        assert_eq!(unix("", &node_server(), EXACT), Hit::No);
        assert_eq!(windows("", &node_server(), EXACT), Hit::No);
        // an empty process name is not the empty pattern's either
        let unnamed = Names::default();
        assert_eq!(unix("", &unnamed, EXACT), Hit::No);
        // --cmd is the substring comparison of `pkill -f`, which any text is in
        assert_eq!(unix("", &node_server(), CMD), Hit::Match);
    }

    #[test]
    fn case_never_matters() {
        assert_eq!(unix("NODE", &node_server(), EXACT), Hit::Match);
        assert_eq!(unix("Node", &node_server(), CMD), Hit::Match);
        let mixed = Names {
            name: "Obsidian",
            ..Names::default()
        };
        assert_eq!(unix("oBSIDIAN", &mixed, EXACT), Hit::Match);
    }

    #[test]
    fn a_word_in_the_command_line_is_not_the_name() {
        // an editor opened on server.js: its command line mentions node
        let editor = Names {
            name: "vim",
            exe: Some("/usr/bin/vim"),
            argv0: Some("vim"),
            command: Some("vim notes-about-node.md"),
        };
        assert_eq!(unix("node", &editor, EXACT), Hit::CommandLineOnly);
        assert!(!matches_name("node", &editor, EXACT));
        assert!(matches_name("node", &editor, CMD));
    }

    #[test]
    fn the_executable_file_name_matches_when_the_name_differs() {
        let versioned = Names {
            name: "python3.12",
            exe: Some("/usr/bin/python3.12"),
            argv0: Some("python"),
            command: Some("python app.py"),
        };
        // by the file name of the executable, never by a directory in its path
        assert_eq!(unix("python3.12", &versioned, EXACT), Hit::Match);
        assert_eq!(unix("bin", &versioned, EXACT), Hit::No);
        assert_eq!(unix("usr", &versioned, EXACT), Hit::No);
        let renamed = Names {
            name: "main",
            exe: Some("/opt/tools/serve"),
            ..Names::default()
        };
        assert_eq!(unix("serve", &renamed, EXACT), Hit::Match);
    }

    #[test]
    fn the_first_argument_file_name_matches_when_the_name_differs() {
        // a script run through its shebang: the name is the script's, the
        // executable the interpreter's, and argv[0] what was typed
        let aliased = Names {
            name: "main",
            exe: Some("/usr/bin/busybox"),
            argv0: Some("./tools/ls"),
            command: Some("./tools/ls -l"),
        };
        assert_eq!(unix("ls", &aliased, EXACT), Hit::Match);
        assert_eq!(unix("tools", &aliased, EXACT), Hit::CommandLineOnly);
        assert_eq!(unix("-l", &aliased, EXACT), Hit::CommandLineOnly);
    }

    #[test]
    fn a_long_name_cut_by_linux_matches_through_the_executable_or_argv0() {
        // Linux cuts the name to 15 characters
        let long = "very-long-service-name";
        let cut = &long[..15];
        let by_exe = Names {
            name: cut,
            exe: Some("/srv/bin/very-long-service-name"),
            argv0: None,
            command: None,
        };
        assert_eq!(unix(long, &by_exe, EXACT), Hit::Match);
        assert_eq!(unix(cut, &by_exe, EXACT), Hit::Match);
        let by_argv0 = Names {
            name: cut,
            exe: None,
            argv0: Some("/srv/bin/very-long-service-name"),
            command: Some("/srv/bin/very-long-service-name --flag"),
        };
        assert_eq!(unix(long, &by_argv0, EXACT), Hit::Match);
        // neither the executable nor argv[0] known: only the cut name is left
        let neither = Names {
            name: cut,
            ..Names::default()
        };
        assert_eq!(unix(long, &neither, EXACT), Hit::No);
        assert_eq!(unix(cut, &neither, EXACT), Hit::Match);
    }

    #[test]
    fn a_missing_executable_or_command_line_is_no_match_not_a_failure() {
        let zombie = Names {
            name: "defunct",
            ..Names::default()
        };
        assert_eq!(unix("defunct", &zombie, EXACT), Hit::Match);
        assert_eq!(unix("node", &zombie, EXACT), Hit::No);
        assert_eq!(unix("node", &zombie, CMD), Hit::No);
        assert_eq!(unix("defun", &zombie, CMD), Hit::Match);
    }

    #[test]
    fn exe_is_optional_on_either_side_only_on_windows() {
        let node = Names {
            name: "node.exe",
            exe: Some(r"C:\Program Files\nodejs\node.exe"),
            argv0: Some(r"C:\Program Files\nodejs\node.exe"),
            command: Some(r#""C:\Program Files\nodejs\node.exe" server.js"#),
        };
        assert_eq!(windows("node", &node, EXACT), Hit::Match);
        assert_eq!(windows("node.exe", &node, EXACT), Hit::Match);
        assert_eq!(windows("NODE.EXE", &node, EXACT), Hit::Match);
        assert_eq!(windows("nodejs", &node, EXACT), Hit::CommandLineOnly);
        assert_eq!(windows("Files", &node, EXACT), Hit::CommandLineOnly);
        // the same process on Unix would be named node.exe, not node
        assert_eq!(unix("node", &node, EXACT), Hit::CommandLineOnly);
        assert_eq!(unix("node.exe", &node, EXACT), Hit::Match);
        // the other way round: the process has no extension, the text has
        let bare = Names {
            name: "node",
            ..Names::default()
        };
        assert_eq!(windows("node.exe", &bare, EXACT), Hit::Match);
        assert_eq!(unix("node.exe", &bare, EXACT), Hit::No);
    }

    #[test]
    fn a_backslash_separates_a_path_only_on_windows() {
        let names = Names {
            name: "x",
            exe: Some(r"C:\tools\serve.exe"),
            ..Names::default()
        };
        assert_eq!(windows("serve", &names, EXACT), Hit::Match);
        // on Unix the whole string is one file name
        assert_eq!(unix("serve", &names, EXACT), Hit::No);
        assert_eq!(unix(r"C:\tools\serve.exe", &names, EXACT), Hit::Match);
        assert_eq!(base_name("/usr/bin/node", false), "node");
        assert_eq!(base_name("node", false), "node");
        assert_eq!(base_name("", false), "");
    }

    #[test]
    fn only_a_trailing_exe_is_dropped() {
        assert_eq!(without_exe("node.exe"), "node");
        assert_eq!(without_exe("NODE.EXE"), "NODE");
        assert_eq!(without_exe("node.exe.bak"), "node.exe.bak");
        assert_eq!(without_exe("exe"), "exe");
        assert_eq!(without_exe(".exe"), "");
        // a multi-byte character where the extension would start
        assert_eq!(without_exe("ünode"), "ünode");
        assert_eq!(without_exe("日本語"), "日本語");
    }

    #[test]
    fn cmd_matches_a_part_of_the_name_or_of_the_command_line() {
        let server = node_server();
        assert_eq!(unix("server.js", &server, CMD), Hit::Match);
        assert_eq!(unix("--PORT 3000", &server, CMD), Hit::Match);
        assert_eq!(unix("nod", &server, CMD), Hit::Match);
        assert_eq!(unix("python", &server, CMD), Hit::No);
        // the executable path is not searched, only the name and the command line
        assert_eq!(unix("local", &server, CMD), Hit::No);
        // with no command line, only the name
        let quiet = Names {
            name: "launchd",
            ..Names::default()
        };
        assert_eq!(unix("launch", &quiet, CMD), Hit::Match);
        assert_eq!(unix("server", &quiet, CMD), Hit::No);
    }

    #[test]
    fn matches_name_follows_this_os() {
        let node = Names {
            name: if cfg!(windows) { "node.exe" } else { "node" },
            ..Names::default()
        };
        assert!(matches_name("node", &node, EXACT));
        assert!(!matches_name("no", &node, EXACT));
        assert!(matches_name("no", &node, CMD));
        assert_eq!(classify("no", &node, EXACT), Hit::CommandLineOnly);
    }
}
