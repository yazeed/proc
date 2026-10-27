//! Target resolution - Convert user input to processes
//!
//! Targets can be:
//! - `:port` - Process listening on this port
//! - `pid` - Process with this PID (numeric)
//! - `name` - Processes matching this name

use crate::core::port::{parse_port, PortInfo};
use crate::core::{NameMatch, Process};
use crate::error::{ProcError, Result};

/// Resolved target type
#[derive(Debug, Clone)]
pub enum TargetType {
    /// Target a process by the port it listens on (e.g., `:3000`)
    Port(u16),
    /// Target a process by its process ID (e.g., `1234`)
    Pid(u32),
    /// Target processes by name (e.g., `node`): the whole name, or with `--cmd`
    /// any part of the name or command line
    Name(String),
}

/// clap value parser for target arguments: a target that starts with ':' must be
/// a port from 1 to 65535. Without this, ":70000" fell through to a name search
/// and reported "not found" instead of a typo.
pub fn validate_target_arg(s: &str) -> std::result::Result<String, String> {
    for target in parse_targets(s) {
        if let Some(port) = target.strip_prefix(':') {
            if !matches!(port.parse::<u16>(), Ok(p) if p > 0) {
                return Err(format!(
                    "'{target}' is not a port; ports go from :1 to :65535"
                ));
            }
        }
    }
    Ok(s.to_string())
}

/// Parse a target string and determine its type
pub fn parse_target(target: &str) -> TargetType {
    let target = target.trim();

    // Explicit port prefix
    if target.starts_with(':') {
        if let Ok(port) = parse_port(target) {
            return TargetType::Port(port);
        }
    }

    // Pure number - treat as PID
    if let Ok(pid) = target.parse::<u32>() {
        return TargetType::Pid(pid);
    }

    // Otherwise it's a name
    TargetType::Name(target.to_string())
}

/// Resolve a target to processes; `mode` says how a name is compared
pub fn resolve_target(target: &str, mode: NameMatch) -> Result<Vec<Process>> {
    match parse_target(target) {
        TargetType::Port(port) => resolve_port(port),
        TargetType::Pid(pid) => resolve_pid(pid),
        TargetType::Name(name) => Process::find_by_name(&name, mode),
    }
}

/// Resolve port to process
fn resolve_port(port: u16) -> Result<Vec<Process>> {
    process_on_port(port, PortInfo::find_by_port(port)?, Process::find_by_pid)
}

/// The process behind `listener`, what was found listening on `port`; `find` looks
/// a process up by PID. The process can end between the two lookups.
fn process_on_port(
    port: u16,
    listener: Option<PortInfo>,
    find: impl FnOnce(u32) -> Result<Option<Process>>,
) -> Result<Vec<Process>> {
    match listener {
        Some(port_info) => match find(port_info.pid)? {
            Some(proc) => Ok(vec![proc]),
            None => Err(ProcError::ProcessGone(port_info.pid)),
        },
        None => Err(ProcError::PortNotFound(port)),
    }
}

/// Resolve PID to process
fn resolve_pid(pid: u32) -> Result<Vec<Process>> {
    match Process::find_by_pid(pid)? {
        Some(proc) => Ok(vec![proc]),
        None => Err(ProcError::ProcessNotFound(pid.to_string())),
    }
}

/// Find all ports a process is listening on
pub fn find_ports_for_pid(pid: u32) -> Result<Vec<PortInfo>> {
    let all_ports = PortInfo::get_all_listening()?;
    Ok(all_ports.into_iter().filter(|p| p.pid == pid).collect())
}

/// Split comma-separated targets into individual target strings
///
/// Examples:
///   ":3000,:8080" -> [":3000", ":8080"]
///   "node,python" -> ["node", "python"]
///   ":3000, 1234, node" -> [":3000", "1234", "node"]
pub fn parse_targets(targets_str: &str) -> Vec<String> {
    targets_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Split every target argument the user typed into individual targets
///
/// A command takes its targets as separate arguments, and a value may still be
/// a comma-separated list, so each value goes through [`parse_targets`].
///
/// Examples:
///   ["123", ":3000,:8080", "node"] -> ["123", ":3000", ":8080", "node"]
pub fn flatten_targets(values: &[String]) -> Vec<String> {
    values.iter().flat_map(|v| parse_targets(v)).collect()
}

/// What resolving several targets came to
#[derive(Debug, Default)]
pub struct Resolved {
    /// The processes found, each once however many targets name it
    pub processes: Vec<Process>,
    /// The targets that matched nothing
    pub not_found: Vec<String>,
    /// For a name that matched nothing, but is in some command lines: what
    /// `--cmd` would find
    pub hints: Vec<String>,
}

impl Resolved {
    /// The warning for the targets that matched nothing, if there were any
    pub fn warning(&self) -> Option<String> {
        if self.not_found.is_empty() {
            return None;
        }
        let mut warning = format!("Not found: {}", self.not_found.join(", "));
        for hint in &self.hints {
            warning.push_str("\n  ");
            warning.push_str(hint);
        }
        Some(warning)
    }
}

/// Resolve multiple targets, deduplicating by PID
pub fn resolve_targets(targets: &[String], mode: NameMatch) -> Resolved {
    resolve_targets_impl(targets, false, mode)
}

/// Resolve multiple targets, excluding the current process (self)
///
/// Use this for destructive commands (kill, stop) to avoid proc killing itself
/// when the target pattern matches proc's own command line arguments.
pub fn resolve_targets_excluding_self(targets: &[String], mode: NameMatch) -> Resolved {
    resolve_targets_impl(targets, true, mode)
}

fn resolve_targets_impl(targets: &[String], exclude_self: bool, mode: NameMatch) -> Resolved {
    use std::collections::HashSet;

    let mut resolved = Resolved::default();
    let mut seen_pids = HashSet::new();
    let self_pid = std::process::id();

    for target in targets {
        match resolve_target(target, mode) {
            Ok(processes) => {
                for proc in processes {
                    // Skip self if requested (for destructive commands)
                    if exclude_self && proc.pid == self_pid {
                        continue;
                    }
                    if seen_pids.insert(proc.pid) {
                        resolved.processes.push(proc);
                    }
                }
            }
            Err(error) => {
                resolved.not_found.push(target.clone());
                resolved.hints.extend(error.command_line_hint());
            }
        }
    }

    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_targets_single() {
        assert_eq!(parse_targets(":3000"), vec![":3000"]);
        assert_eq!(parse_targets("node"), vec!["node"]);
        assert_eq!(parse_targets("1234"), vec!["1234"]);
    }

    #[test]
    fn test_parse_targets_multiple() {
        assert_eq!(parse_targets(":3000,:8080"), vec![":3000", ":8080"]);
        assert_eq!(parse_targets("node,python"), vec!["node", "python"]);
        assert_eq!(
            parse_targets(":3000,1234,node"),
            vec![":3000", "1234", "node"]
        );
    }

    #[test]
    fn test_parse_targets_with_whitespace() {
        assert_eq!(
            parse_targets(":3000, :8080, :9000"),
            vec![":3000", ":8080", ":9000"]
        );
        assert_eq!(parse_targets(" node , python "), vec!["node", "python"]);
    }

    #[test]
    fn separate_arguments_and_comma_lists_flatten_to_one_list_in_order() {
        let typed = |values: &[&str]| -> Vec<String> {
            flatten_targets(&values.iter().map(|v| v.to_string()).collect::<Vec<_>>())
        };
        assert_eq!(typed(&["123", "456"]), vec!["123", "456"]);
        assert_eq!(
            typed(&["123", ":3000,:8080", "node"]),
            vec!["123", ":3000", ":8080", "node"]
        );
        assert_eq!(typed(&["a,b", ",", " c "]), vec!["a", "b", "c"]);
        // a repeated target stays repeated: the PID dedupe happens when resolving
        assert_eq!(typed(&["1", "1,1"]), vec!["1", "1", "1"]);
        assert!(typed(&[]).is_empty());
    }

    #[test]
    fn test_parse_targets_empty_entries() {
        assert_eq!(parse_targets(":3000,,,:8080"), vec![":3000", ":8080"]);
        assert_eq!(parse_targets(",,node,,"), vec!["node"]);
    }

    #[test]
    fn test_parse_target_port() {
        assert!(matches!(parse_target(":3000"), TargetType::Port(3000)));
        assert!(matches!(parse_target(":8080"), TargetType::Port(8080)));
    }

    #[test]
    fn test_parse_target_pid() {
        assert!(matches!(parse_target("1234"), TargetType::Pid(1234)));
        assert!(matches!(parse_target("99999"), TargetType::Pid(99999)));
    }

    #[test]
    fn a_colon_target_that_is_not_a_port_is_a_name() {
        for target in [":abc", ":70000", ":"] {
            assert!(
                matches!(parse_target(target), TargetType::Name(n) if n == target),
                "{target}"
            );
        }
    }

    #[test]
    fn test_parse_target_name() {
        assert!(matches!(parse_target("node"), TargetType::Name(_)));
        assert!(matches!(parse_target("my-process"), TargetType::Name(_)));
    }

    #[test]
    fn only_a_colon_target_has_to_be_a_port() {
        // names and PIDs are left alone, alone or in a list with ports
        assert_eq!(validate_target_arg("node").unwrap(), "node");
        assert_eq!(validate_target_arg("1234").unwrap(), "1234");
        assert_eq!(validate_target_arg(":3000, node").unwrap(), ":3000, node");
        for bad in [":70000", ":0", ":abc", "node,:70000"] {
            let err = validate_target_arg(bad).unwrap_err();
            assert!(err.contains("is not a port"), "{bad}: {err}");
        }
    }

    fn listener(pid: u32) -> PortInfo {
        PortInfo {
            port: 4000,
            protocol: crate::core::Protocol::Tcp,
            pid,
            process_name: "x".to_string(),
            address: None,
        }
    }

    fn own_process() -> Process {
        Process::find_by_pid(std::process::id()).unwrap().unwrap()
    }

    #[test]
    fn a_port_resolves_to_the_process_listening_on_it() {
        let found = process_on_port(4000, Some(listener(7)), |pid| {
            assert_eq!(pid, 7);
            Ok(Some(own_process()))
        })
        .unwrap();
        assert_eq!(found.len(), 1);

        let none = process_on_port(4000, None, |_| panic!("no listener, no lookup"));
        assert!(
            matches!(none, Err(ProcError::PortNotFound(4000))),
            "{none:?}"
        );

        // the process ended after the port was listed
        let gone = process_on_port(4000, Some(listener(7)), |_| Ok(None));
        assert!(matches!(gone, Err(ProcError::ProcessGone(7))), "{gone:?}");

        let failed = process_on_port(4000, Some(listener(7)), |_| {
            Err(ProcError::SystemError("no".to_string()))
        });
        assert!(
            matches!(failed, Err(ProcError::SystemError(_))),
            "{failed:?}"
        );
    }

    #[test]
    fn targets_resolve_once_each_and_never_to_ourselves_when_asked() {
        let me = std::process::id().to_string();
        let targets = vec![me.clone(), me.clone(), "99999999".to_string()];

        // listed twice, found once; the made-up PID is not found
        let resolved = resolve_targets(&targets, NameMatch::Exact);
        assert_eq!(resolved.processes.len(), 1);
        assert_eq!(resolved.processes[0].pid, std::process::id());
        assert_eq!(resolved.not_found, vec!["99999999".to_string()]);
        assert_eq!(
            resolved.warning().unwrap(),
            "Not found: 99999999",
            "a PID has no --cmd hint"
        );

        let resolved = resolve_targets_excluding_self(&targets, NameMatch::Exact);
        assert!(resolved.processes.is_empty());
        assert_eq!(resolved.not_found, vec!["99999999".to_string()]);
    }

    #[test]
    fn nothing_missing_means_no_warning() {
        let me = std::process::id().to_string();
        let resolved = resolve_targets(&[me], NameMatch::Exact);
        assert!(resolved.not_found.is_empty());
        assert_eq!(resolved.warning(), None);
    }
}
