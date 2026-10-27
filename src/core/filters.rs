//! Shared filter utilities used across commands
//!
//! Provides common filter resolution logic to avoid duplication.

use crate::core::{Process, ProcessStatus};
use clap::ValueEnum;
use std::path::{Path, PathBuf};

/// Values accepted by `--status`. An unknown value is a usage error (exit 4),
/// not a filter that silently matches everything.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum StatusFilter {
    /// Running on a CPU
    Running,
    /// Waiting (idle, blocked on I/O)
    #[value(alias = "sleep")]
    Sleeping,
    /// Paused (e.g. by `proc freeze` or Ctrl+Z)
    #[value(alias = "stop")]
    Stopped,
    /// Exited, but not yet reaped by its parent
    Zombie,
}

impl StatusFilter {
    /// Whether a process with this status passes the filter
    pub fn matches(self, status: ProcessStatus) -> bool {
        matches!(
            (self, status),
            (StatusFilter::Running, ProcessStatus::Running)
                | (StatusFilter::Sleeping, ProcessStatus::Sleeping)
                | (StatusFilter::Stopped, ProcessStatus::Stopped)
                | (StatusFilter::Zombie, ProcessStatus::Zombie)
        )
    }
}

/// Sort key for process commands (list, by, in, for)
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SortKey {
    /// Sort by CPU usage (descending)
    Cpu,
    /// Sort by memory usage (descending)
    Mem,
    /// Sort by process ID (ascending)
    Pid,
    /// Sort by process name (ascending)
    Name,
}

/// Sort key for port commands
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PortSortKey {
    /// Sort by port number (ascending)
    Port,
    /// Sort by process ID (ascending)
    Pid,
    /// Sort by process name (ascending)
    Name,
}

/// Sort a list of processes by the given key.
pub fn sort_processes(processes: &mut [Process], key: SortKey) {
    match key {
        SortKey::Cpu => processes.sort_by(|a, b| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        SortKey::Mem => processes.sort_by(|a, b| {
            b.memory_mb
                .partial_cmp(&a.memory_mb)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        SortKey::Pid => processes.sort_by_key(|p| p.pid),
        SortKey::Name => processes.sort_by_key(|p| p.name.to_lowercase()),
    }
}

/// Check if a process matches a `--by` name filter.
///
/// Matches against both the process name AND full command line (case-insensitive),
/// consistent with how positional targets work via `find_by_name`.
pub fn matches_by_filter(process: &Process, pattern: &str) -> bool {
    let pattern_lower = pattern.to_lowercase();
    if process.name.to_lowercase().contains(&pattern_lower) {
        return true;
    }
    if let Some(ref cmd) = process.command {
        if cmd.to_lowercase().contains(&pattern_lower) {
            return true;
        }
    }
    false
}

/// Check if a process matches an `--in` directory filter.
pub fn matches_in_filter(process: &Process, dir_path: &std::path::Path) -> bool {
    if let Some(ref cwd) = process.cwd {
        cwd_within(cwd, dir_path)
    } else {
        false
    }
}

/// Whether a process's cwd is `dir` or under it.
///
/// `dir` is already in the OS's own form (see [`resolve_dir`]). A Windows cwd
/// is not always: it comes back in whatever form the process was started
/// with. Measured on a GitHub runner, a process spawned with cwd
/// `C:\Users\RUNNER~1\AppData\Local\Temp\.tmpX` reports
/// `"cwd": "C:\\Users\\RUNNER~1\\AppData\\Local\\Temp\\.tmpX\\"` (short name) while its
/// `exe_path` is `C:\Users\runneradmin\...` (long). So when the plain
/// comparison fails, on Windows the cwd is canonicalized too and compared again.
pub fn cwd_within(cwd: &str, dir: &Path) -> bool {
    #[cfg(windows)]
    let canonicalize = |p: &Path| p.canonicalize().ok();
    #[cfg(not(windows))]
    let canonicalize = |_: &Path| None;
    cwd_within_with(cwd, dir, cfg!(windows), canonicalize)
}

/// [`cwd_within`] with the canonicalizer and the platform switch as inputs,
/// so every OS can test the Windows path with a fake canonicalizer.
fn cwd_within_with(
    cwd: &str,
    dir: &Path,
    retry_canonical: bool,
    canonicalize: impl Fn(&Path) -> Option<PathBuf>,
) -> bool {
    if Path::new(cwd).starts_with(dir) {
        return true;
    }
    if !retry_canonical {
        return false;
    }
    match canonicalize(Path::new(cwd)) {
        Some(canonical) => {
            PathBuf::from(strip_verbatim_prefix(&canonical.to_string_lossy())).starts_with(dir)
        }
        None => false,
    }
}

/// Apply `--in` and `--by` filters to a list of processes (in a single pass).
///
/// This replaces the 14-line retain block duplicated across commands.
pub fn apply_filters(
    processes: &mut Vec<Process>,
    in_dir: &Option<String>,
    by_name: &Option<String>,
) {
    let in_dir_filter = resolve_in_dir(in_dir);
    processes.retain(|p| {
        if let Some(ref dir_path) = in_dir_filter {
            if !matches_in_filter(p, dir_path) {
                return false;
            }
        }
        if let Some(ref name) = by_name {
            if !matches_by_filter(p, name) {
                return false;
            }
        }
        true
    });
}

/// Resolve an `--in` directory filter to the form processes report their cwd in.
///
/// Handles "." (current directory), relative paths, and absolute paths, then
/// resolves symlinks (see [`resolve_dir`]).
pub fn resolve_in_dir(in_dir: &Option<String>) -> Option<PathBuf> {
    in_dir.as_ref().map(|p| {
        if p == "." {
            resolve_dir(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        } else {
            resolve_dir(PathBuf::from(p))
        }
    })
}

/// Make a directory path comparable with the cwds the OS reports.
///
/// A relative path is joined onto the current directory. If the directory
/// exists it is canonicalized: the OS reports a process's cwd with symlinks
/// resolved (`/tmp/x` is `/private/tmp/x` on macOS) and, on Windows, with the
/// real case, long names and plain `D:\` form, so the path the user typed
/// (symlinked, `RUNNER~1`, forward slashes) must be put in that form before
/// the two are compared. A directory that does not exist (it may still be the
/// cwd of a process that outlived it) is only made absolute.
pub fn resolve_dir(path: PathBuf) -> PathBuf {
    let absolute = if path.is_relative() {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    } else {
        path
    };
    match absolute.canonicalize() {
        // Windows returns `\\?\D:\...`, but processes report `D:\...`
        #[cfg(windows)]
        Ok(canonical) => PathBuf::from(strip_verbatim_prefix(&canonical.to_string_lossy())),
        #[cfg(not(windows))]
        Ok(canonical) => canonical,
        Err(_) => absolute,
    }
}

/// Turn a Windows verbatim path into the plain form processes report.
///
/// `canonicalize` on Windows returns `\\?\D:\app\x.exe` (or
/// `\\?\UNC\server\share\x.exe`), while a process's executable path is
/// `D:\app\x.exe` (`\\server\share\x.exe`), so the two never compare equal.
/// Any other form comes back unchanged. Pure string work, so it is tested on
/// every OS; only Windows calls it.
pub fn strip_verbatim_prefix(path: &str) -> String {
    if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{unc}");
    }
    if let Some(disk) = path.strip_prefix(r"\\?\") {
        let mut chars = disk.chars();
        let is_disk = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            && chars.next() == Some(':')
            && matches!(chars.next(), Some('\\') | None);
        if is_disk {
            return disk.to_string();
        }
    }
    path.to_string()
}

#[cfg(test)]
mod verbatim_tests {
    use super::strip_verbatim_prefix;

    #[test]
    fn a_verbatim_disk_path_loses_its_prefix() {
        assert_eq!(
            strip_verbatim_prefix(r"\\?\D:\a\proc-cli\listener.exe"),
            r"D:\a\proc-cli\listener.exe"
        );
    }

    #[test]
    fn a_verbatim_unc_path_becomes_a_plain_unc_path() {
        assert_eq!(
            strip_verbatim_prefix(r"\\?\UNC\server\share\listener.exe"),
            r"\\server\share\listener.exe"
        );
    }

    #[test]
    fn other_forms_are_unchanged() {
        for path in [
            r"D:\a\proc-cli\listener.exe",
            r"\\server\share\listener.exe",
            r"\\?\Volume{0123}\listener.exe",
            "/usr/bin/node",
            "",
        ] {
            assert_eq!(strip_verbatim_prefix(path), path);
        }
    }
}

#[cfg(test)]
mod resolve_tests {
    use super::{resolve_dir, resolve_in_dir};
    use std::path::PathBuf;

    #[test]
    fn no_filter_resolves_to_nothing() {
        assert_eq!(resolve_in_dir(&None), None);
    }

    #[test]
    fn a_dot_is_the_current_directory() {
        let cwd = std::env::current_dir().unwrap().canonicalize().unwrap();
        let resolved = resolve_in_dir(&Some(".".to_string())).unwrap();
        assert_eq!(resolved.canonicalize().unwrap(), cwd);
        assert!(resolved.is_absolute());
    }

    #[test]
    fn a_relative_path_is_taken_from_the_current_directory() {
        // cargo runs unit tests with the package root as the current directory
        let expected = std::env::current_dir()
            .unwrap()
            .join("src")
            .canonicalize()
            .unwrap();
        let resolved = resolve_in_dir(&Some("src".to_string())).unwrap();
        assert_eq!(resolved.canonicalize().unwrap(), expected);
        assert!(resolved.is_absolute());
    }

    #[test]
    fn a_missing_relative_path_is_only_made_absolute() {
        let expected = std::env::current_dir().unwrap().join("no-such-dir-8f3a");
        assert_eq!(
            resolve_in_dir(&Some("no-such-dir-8f3a".to_string())),
            Some(expected)
        );
    }

    #[test]
    fn a_missing_absolute_path_is_kept_as_written() {
        let tmp = tempfile::tempdir().unwrap();
        let gone = tmp.path().join("gone");
        assert_eq!(resolve_dir(gone.clone()), gone);
    }

    #[test]
    fn an_existing_directory_comes_back_in_canonical_form() {
        let tmp = tempfile::tempdir().unwrap();
        let expected = PathBuf::from(super::strip_verbatim_prefix(
            &tmp.path().canonicalize().unwrap().to_string_lossy(),
        ));
        assert_eq!(resolve_dir(tmp.path().to_path_buf()), expected);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_is_resolved_to_its_target() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("real");
        std::fs::create_dir(&target).unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert_eq!(resolve_dir(link), target.canonicalize().unwrap());
    }
}

#[cfg(test)]
mod match_tests {
    use super::{matches_by_filter, matches_in_filter};
    use crate::core::{Process, ProcessStatus};
    use std::path::Path;

    /// A process record with no working directory and no command line, which
    /// some processes really are (a zombie has neither on Linux)
    fn bare_process() -> Process {
        Process {
            pid: 1,
            name: "Worker".to_string(),
            exe_path: None,
            cwd: None,
            command: None,
            cpu_percent: 0.0,
            memory_mb: 0.0,
            status: ProcessStatus::Sleeping,
            user: None,
            parent_pid: None,
            start_time: None,
            uptime_secs: None,
        }
    }

    #[test]
    fn a_process_with_no_working_directory_is_not_in_any_directory() {
        assert!(!matches_in_filter(&bare_process(), Path::new("/")));
    }

    #[test]
    fn a_working_directory_matches_itself_and_what_is_under_it() {
        let mut process = bare_process();
        process.cwd = Some("/srv/app/api".to_string());
        assert!(matches_in_filter(&process, Path::new("/srv/app")));
        assert!(matches_in_filter(&process, Path::new("/srv/app/api")));
        assert!(!matches_in_filter(&process, Path::new("/srv/ap")));
        assert!(!matches_in_filter(&process, Path::new("/srv/app/web")));
    }

    #[test]
    fn a_name_matches_by_name_or_by_command_line_ignoring_case() {
        let mut process = bare_process();
        assert!(matches_by_filter(&process, "work"));
        assert!(!matches_by_filter(&process, "node"));
        process.command = Some("/usr/bin/Node server.js".to_string());
        assert!(matches_by_filter(&process, "node"));
        assert!(!matches_by_filter(&process, "python"));
    }
}

#[cfg(test)]
mod cwd_within_tests {
    use super::cwd_within_with;
    use std::path::{Path, PathBuf};

    /// Maps the short name to the long one, as Windows canonicalize does
    /// (forward slashes, so the paths mean the same on every OS)
    fn fake(p: &Path) -> Option<PathBuf> {
        let s = p.to_string_lossy().replace("RUNNER~1", "runneradmin");
        s.starts_with("C:").then(|| PathBuf::from(s))
    }

    #[test]
    fn a_plain_match_needs_no_canonicalizing() {
        assert!(cwd_within_with(
            "/srv/app/api",
            Path::new("/srv/app"),
            false,
            |_| None
        ));
        assert!(!cwd_within_with(
            "/srv/other",
            Path::new("/srv/app"),
            false,
            |_| None
        ));
    }

    #[test]
    fn a_short_name_cwd_matches_the_long_name_dir_when_retrying() {
        let cwd = "C:/Users/RUNNER~1/Temp/.tmpX/";
        let dir = Path::new("C:/Users/runneradmin/Temp/.tmpX");
        assert!(cwd_within_with(cwd, dir, true, fake));
        assert!(!cwd_within_with(cwd, dir, false, fake));
    }

    #[test]
    fn a_cwd_that_cannot_be_canonicalized_does_not_match() {
        let dir = Path::new("C:/Users/runneradmin/Temp");
        assert!(!cwd_within_with("D:/gone", dir, true, fake));
    }

    #[test]
    fn a_canonical_cwd_outside_the_dir_does_not_match() {
        let dir = Path::new("C:/Users/runneradmin/Temp");
        assert!(!cwd_within_with("C:/Users/RUNNER~1/Other", dir, true, fake));
    }
}
