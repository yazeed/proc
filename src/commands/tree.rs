//! Tree command - Show process tree
//!
//! Usage:
//!   proc tree              # Full process tree
//!   proc tree node         # Tree for node processes
//!   proc tree :3000        # Tree for process on port 3000
//!   proc tree 1234         # Tree for PID 1234
//!   proc tree --min-cpu 10 # Only processes using >10% CPU
//!   proc tree 1234 -a      # Show ancestry (path UP to root)

use crate::core::{
    matches_by_filter, matches_in_filter, parse_target, resolve_in_dir, resolve_target, Process,
    ProcessStatus, StatusFilter, TargetType,
};
use crate::error::Result;
use crate::ui::{format_memory, plural, uptime_label, Printer};
use clap::Args;
use colored::*;
use serde::Serialize;
use std::collections::HashMap;

/// Show process tree
#[derive(Args, Debug)]
pub struct TreeCommand {
    /// Target: process name, :port, or PID (shows full tree if omitted)
    #[arg(value_parser = crate::core::validate_target_arg)]
    target: Option<String>,

    /// Show ancestry (path UP to root) instead of descendants
    #[arg(long, short)]
    ancestors: bool,

    /// Show verbose output
    #[arg(long, short = 'v')]
    verbose: bool,

    /// Output as JSON
    #[arg(long, short = 'j')]
    json: bool,

    /// Maximum depth to display
    #[arg(long, short, default_value = "10")]
    depth: usize,

    /// Show PIDs only (compact view)
    #[arg(long, short = 'C')]
    compact: bool,

    /// Only show processes using more than this CPU %
    #[arg(long)]
    min_cpu: Option<f32>,

    /// Only show processes using more than this memory (MB)
    #[arg(long)]
    min_mem: Option<f64>,

    /// Filter by status: running, sleeping, stopped, zombie
    #[arg(long, value_enum, ignore_case = true)]
    status: Option<StatusFilter>,

    /// Only show processes running longer than this (seconds)
    #[arg(long)]
    min_uptime: Option<u64>,

    /// Filter by directory; takes the next word as the directory unless it starts with '-'
    /// (bare --in is the current directory, --in=DIR is explicit)
    #[arg(long = "in", short = 'i', num_args = 0..=1, default_missing_value = ".")]
    pub in_dir: Option<String>,

    /// Filter by process name
    #[arg(long = "by", short = 'b')]
    pub by_name: Option<String>,
}

impl TreeCommand {
    /// Build the set of ancestor PIDs to exclude from name matching.
    ///
    /// Walks up the process tree from the current process to avoid matching
    /// shells whose command lines contain proc's own arguments. `all_processes`
    /// leaves proc itself out, so its parent is looked up separately.
    fn ancestor_pids(all_processes: &[Process]) -> std::collections::HashSet<u32> {
        let mut pids = std::collections::HashSet::new();
        let self_pid = std::process::id();
        pids.insert(self_pid);
        let mut next = Process::find_by_pid(self_pid)
            .ok()
            .flatten()
            .and_then(|me| me.parent_pid);
        for _ in 0..10 {
            let Some(parent_pid) = next else { break };
            pids.insert(parent_pid);
            next = all_processes
                .iter()
                .find(|p| p.pid == parent_pid)
                .and_then(|p| p.parent_pid);
        }
        pids
    }

    /// Executes the tree command, displaying the process hierarchy.
    pub fn execute(&self) -> Result<()> {
        let printer = Printer::from_flags(self.json, self.verbose);

        // Get all processes, with real CPU readings for the nodes and --min-cpu
        let mut all_processes = Process::find_all()?;
        Process::measure_cpu(&mut all_processes);

        // Build PID -> Process map for quick lookup
        let pid_map: HashMap<u32, &Process> = all_processes.iter().map(|p| (p.pid, p)).collect();

        // Build parent -> children map
        let mut children_map: HashMap<u32, Vec<&Process>> = HashMap::new();

        for proc in &all_processes {
            if let Some(ppid) = proc.parent_pid {
                children_map.entry(ppid).or_default().push(proc);
            }
        }

        // Handle --ancestors mode
        if self.ancestors {
            return self.show_ancestors(&printer, &pid_map);
        }

        // Determine target processes
        let target_processes: Vec<&Process> = if let Some(ref target) = self.target {
            // Use unified target resolution
            match parse_target(target) {
                TargetType::Port(_) | TargetType::Pid(_) => {
                    // For port or PID, resolve to specific process(es)
                    // Errors (not found) rather than returning nothing
                    let resolved = resolve_target(target)?;
                    // Find matching processes in all_processes
                    let pids: Vec<u32> = resolved.iter().map(|p| p.pid).collect();
                    all_processes
                        .iter()
                        .filter(|p| pids.contains(&p.pid))
                        .collect()
                }
                TargetType::Name(ref pattern) => {
                    // For name, do pattern matching (exclude self and parent to avoid
                    // false positives from proc's own args in the shell command line)
                    let ancestor_pids = Self::ancestor_pids(&all_processes);
                    all_processes
                        .iter()
                        .filter(|p| {
                            !ancestor_pids.contains(&p.pid) && matches_by_filter(p, pattern)
                        })
                        .collect()
                }
            }
        } else {
            Vec::new() // Will show full tree
        };

        // Apply --in and --by filters (only for targeted mode)
        let target_processes = if self.target.is_some() {
            let in_dir_filter = resolve_in_dir(&self.in_dir);
            target_processes
                .into_iter()
                .filter(|p| {
                    if let Some(ref dir_path) = in_dir_filter {
                        if !matches_in_filter(p, dir_path) {
                            return false;
                        }
                    }
                    if let Some(ref name) = self.by_name {
                        if !matches_by_filter(p, name) {
                            return false;
                        }
                    }
                    true
                })
                .collect()
        } else {
            target_processes
        };

        // Apply resource filters if specified
        let matches_filters = |p: &Process| self.matches_filters(p);

        // Apply filters to target processes or find filtered roots
        let has_filters = self.min_cpu.is_some()
            || self.min_mem.is_some()
            || self.status.is_some()
            || self.min_uptime.is_some();

        if self.json {
            let tree_nodes = if self.target.is_some() {
                target_processes
                    .iter()
                    .filter(|p| matches_filters(p))
                    .map(|p| self.build_tree_node(p, &children_map, 0))
                    .collect()
            } else if has_filters {
                // Show only processes matching filters
                all_processes
                    .iter()
                    .filter(|p| matches_filters(p))
                    .map(|p| self.build_tree_node(p, &children_map, 0))
                    .collect()
            } else {
                // Show full tree from roots
                all_processes
                    .iter()
                    .filter(|p| p.parent_pid.is_none() || p.parent_pid == Some(0))
                    .map(|p| self.build_tree_node(p, &children_map, 0))
                    .collect()
            };

            printer.print_json(&TreeOutput {
                action: "tree",
                success: true,
                tree: tree_nodes,
            });
        } else if self.target.is_some() {
            let filtered: Vec<_> = target_processes
                .into_iter()
                .filter(|p| matches_filters(p))
                .collect();
            if filtered.is_empty() {
                printer.warning(&format!(
                    "No processes found for '{}'",
                    self.target.as_ref().unwrap()
                ));
                return Ok(());
            }

            println!(
                "{} Process tree for '{}':\n",
                "✓".green().bold(),
                self.target.as_ref().unwrap().cyan()
            );

            for proc in &filtered {
                self.print_tree(proc, &children_map, "", true, 0);
                println!();
            }
        } else if has_filters {
            let filtered: Vec<_> = all_processes
                .iter()
                .filter(|p| matches_filters(p))
                .collect();
            if filtered.is_empty() {
                printer.warning("No processes match the specified filters");
                return Ok(());
            }

            println!(
                "{} {} process{} matching filters:\n",
                "✓".green().bold(),
                filtered.len().to_string().cyan().bold(),
                plural(filtered.len())
            );

            for (i, proc) in filtered.iter().enumerate() {
                let is_last = i == filtered.len() - 1;
                self.print_tree(proc, &children_map, "", is_last, 0);
            }
        } else {
            println!("{} Process tree:\n", "✓".green().bold());

            // Find processes with PID 1 or no parent as roots
            let display_roots: Vec<&Process> = all_processes
                .iter()
                .filter(|p| p.parent_pid.is_none() || p.parent_pid == Some(0))
                .collect();

            for (i, proc) in display_roots.iter().enumerate() {
                let is_last = i == display_roots.len() - 1;
                self.print_tree(proc, &children_map, "", is_last, 0);
            }
        }

        Ok(())
    }

    /// Whether a process passes `--min-cpu`, `--min-mem`, `--status` and `--min-uptime`
    fn matches_filters(&self, p: &Process) -> bool {
        if let Some(min_cpu) = self.min_cpu {
            if p.cpu_percent < min_cpu {
                return false;
            }
        }
        if let Some(min_mem) = self.min_mem {
            if p.memory_mb < min_mem {
                return false;
            }
        }
        if let Some(status) = self.status {
            if !status.matches(p.status) {
                return false;
            }
        }
        if let Some(min_uptime) = self.min_uptime {
            if let Some(start_time) = p.start_time {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                if now.saturating_sub(start_time) < min_uptime {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }

    fn print_tree(
        &self,
        proc: &Process,
        children_map: &HashMap<u32, Vec<&Process>>,
        prefix: &str,
        is_last: bool,
        depth: usize,
    ) {
        if depth > self.depth {
            return;
        }

        let connector = if is_last { "└── " } else { "├── " };

        if self.compact {
            println!(
                "{}{}{}",
                prefix.bright_black(),
                connector.bright_black(),
                proc.pid.to_string().cyan()
            );
        } else {
            let status_indicator = status_indicator(proc.status);

            println!(
                "{}{}{} {} [{}] {:.1}% {}{}",
                prefix.bright_black(),
                connector.bright_black(),
                status_indicator,
                proc.name.white().bold(),
                proc.pid.to_string().cyan(),
                proc.cpu_percent,
                format_memory(proc.memory_mb),
                uptime_label(proc.uptime_secs).bright_black()
            );
        }

        let child_prefix = if is_last {
            format!("{}    ", prefix)
        } else {
            format!("{}│   ", prefix)
        };

        if let Some(children) = children_map.get(&proc.pid) {
            let mut sorted_children: Vec<&&Process> = children.iter().collect();
            sorted_children.sort_by_key(|p| p.pid);

            for (i, child) in sorted_children.iter().enumerate() {
                let child_is_last = i == sorted_children.len() - 1;
                self.print_tree(child, children_map, &child_prefix, child_is_last, depth + 1);
            }
        }
    }

    fn build_tree_node(
        &self,
        proc: &Process,
        children_map: &HashMap<u32, Vec<&Process>>,
        depth: usize,
    ) -> TreeNode {
        let children = if depth < self.depth {
            children_map
                .get(&proc.pid)
                .map(|kids| {
                    kids.iter()
                        .map(|p| self.build_tree_node(p, children_map, depth + 1))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        TreeNode {
            pid: proc.pid,
            name: proc.name.clone(),
            cpu_percent: proc.cpu_percent,
            memory_mb: proc.memory_mb,
            status: proc.status,
            uptime_secs: proc.uptime_secs,
            children,
        }
    }

    /// Show ancestry (path UP to root) for target processes
    fn show_ancestors(&self, printer: &Printer, pid_map: &HashMap<u32, &Process>) -> Result<()> {
        use crate::core::{parse_target, resolve_target, TargetType};

        let target = match &self.target {
            Some(t) => t,
            None => {
                printer.warning("--ancestors requires a target (PID, :port, or name)");
                return Ok(());
            }
        };

        // Resolve target to processes
        let target_processes = match parse_target(target) {
            TargetType::Port(_) | TargetType::Pid(_) => resolve_target(target)?,
            TargetType::Name(ref pattern) => {
                let all_procs: Vec<Process> = pid_map.values().map(|p| (*p).clone()).collect();
                let ancestor_pids = Self::ancestor_pids(&all_procs);
                pid_map
                    .values()
                    .filter(|p| !ancestor_pids.contains(&p.pid) && matches_by_filter(p, pattern))
                    .map(|p| (*p).clone())
                    .collect()
            }
        };

        if target_processes.is_empty() {
            printer.warning(&format!("No process found for '{}'", target));
            return Ok(());
        }

        if self.json {
            let ancestry_output: Vec<AncestryNode> = target_processes
                .iter()
                .map(|proc| self.build_ancestry_node(proc, pid_map))
                .collect();
            printer.print_json(&AncestryOutput {
                action: "tree",
                success: true,
                ancestry: ancestry_output,
            });
        } else {
            println!("{} Ancestry for '{}':\n", "✓".green().bold(), target.cyan());

            for proc in &target_processes {
                self.print_ancestry(proc, pid_map);
                println!();
            }
        }

        Ok(())
    }

    /// Trace and print ancestry from root down to target
    fn print_ancestry(&self, target: &Process, pid_map: &HashMap<u32, &Process>) {
        // Root first, target last
        let chain = ancestor_chain(target, pid_map);

        // Print the chain
        for (i, proc) in chain.iter().enumerate() {
            let is_target = proc.pid == target.pid;
            let indent = "    ".repeat(i);
            let connector = if i == 0 { "" } else { "└── " };

            let status_indicator = status_indicator(proc.status);

            if is_target {
                // Highlight the target
                println!(
                    "{}{}{} {} [{}] {:.1}% {}{}  {}",
                    indent.bright_black(),
                    connector.bright_black(),
                    status_indicator,
                    proc.name.cyan().bold(),
                    proc.pid.to_string().cyan().bold(),
                    proc.cpu_percent,
                    format_memory(proc.memory_mb),
                    uptime_label(proc.uptime_secs).bright_black(),
                    "← target".yellow()
                );
            } else {
                println!(
                    "{}{}{} {} [{}] {:.1}% {}{}",
                    indent.bright_black(),
                    connector.bright_black(),
                    status_indicator,
                    proc.name.white(),
                    proc.pid.to_string().cyan(),
                    proc.cpu_percent,
                    format_memory(proc.memory_mb),
                    uptime_label(proc.uptime_secs).bright_black()
                );
            }
        }
    }

    /// Build ancestry node for JSON output
    fn build_ancestry_node(
        &self,
        target: &Process,
        pid_map: &HashMap<u32, &Process>,
    ) -> AncestryNode {
        let chain: Vec<ProcessInfo> = ancestor_chain(target, pid_map)
            .into_iter()
            .map(|proc| ProcessInfo {
                pid: proc.pid,
                name: proc.name.clone(),
                cpu_percent: proc.cpu_percent,
                memory_mb: proc.memory_mb,
                status: proc.status,
                uptime_secs: proc.uptime_secs,
            })
            .collect();

        AncestryNode {
            target_pid: target.pid,
            target_name: target.name.clone(),
            depth: chain.len(),
            chain,
        }
    }
}

/// The marker for a process status in the tree
fn status_indicator(status: ProcessStatus) -> ColoredString {
    match status {
        ProcessStatus::Running => "●".green(),
        ProcessStatus::Sleeping => "○".blue(),
        ProcessStatus::Stopped => "◐".yellow(),
        ProcessStatus::Zombie => "✗".red(),
        _ => "?".white(),
    }
}

/// The processes from the root down to `target`, found by following parent
/// PIDs. The chain ends at a parent the process list doesn't hold (the root's
/// parent is 0, or the parent is gone) and is cut at 100 so a loop in the
/// parent links cannot run forever.
fn ancestor_chain<'a>(
    target: &'a Process,
    pid_map: &HashMap<u32, &'a Process>,
) -> Vec<&'a Process> {
    let mut chain: Vec<&Process> = Vec::new();
    let mut current_pid = Some(target.pid);

    while let Some(pid) = current_pid {
        let Some(proc) = pid_map.get(&pid) else { break };
        chain.push(proc);
        current_pid = proc.parent_pid;
        if chain.len() > 100 {
            break;
        }
    }

    chain.reverse();
    chain
}

#[derive(Serialize)]
struct AncestryOutput {
    action: &'static str,
    success: bool,
    ancestry: Vec<AncestryNode>,
}

#[derive(Serialize)]
struct AncestryNode {
    target_pid: u32,
    target_name: String,
    depth: usize,
    chain: Vec<ProcessInfo>,
}

#[derive(Serialize)]
struct ProcessInfo {
    pid: u32,
    name: String,
    cpu_percent: f32,
    memory_mb: f64,
    status: ProcessStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    uptime_secs: Option<u64>,
}

#[derive(Serialize)]
struct TreeOutput {
    action: &'static str,
    success: bool,
    tree: Vec<TreeNode>,
}

#[derive(Serialize)]
struct TreeNode {
    pid: u32,
    name: String,
    cpu_percent: f32,
    memory_mb: f64,
    status: ProcessStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    uptime_secs: Option<u64>,
    children: Vec<TreeNode>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command() -> TreeCommand {
        TreeCommand {
            target: None,
            ancestors: false,
            verbose: false,
            json: false,
            depth: 10,
            compact: false,
            min_cpu: None,
            min_mem: None,
            status: None,
            min_uptime: None,
            in_dir: None,
            by_name: None,
        }
    }

    /// A process record with no start time, which some processes really are
    fn process(pid: u32, parent_pid: Option<u32>) -> Process {
        Process {
            pid,
            name: format!("p{pid}"),
            exe_path: None,
            cwd: None,
            command: None,
            cpu_percent: 0.0,
            memory_mb: 0.0,
            status: ProcessStatus::Sleeping,
            user: None,
            parent_pid,
            start_time: None,
            uptime_secs: None,
        }
    }

    #[test]
    fn a_process_with_no_start_time_fails_min_uptime() {
        let cmd = TreeCommand {
            min_uptime: Some(0),
            ..command()
        };
        assert!(!cmd.matches_filters(&process(1, None)));
        assert!(command().matches_filters(&process(1, None)));
    }

    #[test]
    fn every_status_has_its_own_marker() {
        let color = |status| status_indicator(status).fgcolor;
        let text = |status| status_indicator(status).trim().to_string();
        assert_eq!(text(ProcessStatus::Running), "●");
        assert_eq!(text(ProcessStatus::Sleeping), "○");
        assert_eq!(text(ProcessStatus::Stopped), "◐");
        assert_eq!(text(ProcessStatus::Zombie), "✗");
        assert_eq!(text(ProcessStatus::Dead), "?");
        assert_eq!(text(ProcessStatus::Unknown), "?");
        assert_eq!(color(ProcessStatus::Running), Some(Color::Green));
        assert_eq!(color(ProcessStatus::Sleeping), Some(Color::Blue));
        assert_eq!(color(ProcessStatus::Stopped), Some(Color::Yellow));
        assert_eq!(color(ProcessStatus::Zombie), Some(Color::Red));
        assert_eq!(color(ProcessStatus::Unknown), Some(Color::White));
    }

    #[test]
    fn the_chain_runs_from_the_root_down_to_the_target() {
        let (root, middle, leaf) = (
            process(1, Some(0)),
            process(2, Some(1)),
            process(3, Some(2)),
        );
        let map: HashMap<u32, &Process> = [&root, &middle, &leaf]
            .iter()
            .map(|p| (p.pid, *p))
            .collect();
        let chain: Vec<u32> = ancestor_chain(&leaf, &map).iter().map(|p| p.pid).collect();
        assert_eq!(chain, vec![1, 2, 3]);
    }

    #[test]
    fn a_loop_in_the_parent_links_is_cut() {
        let (a, b) = (process(1, Some(2)), process(2, Some(1)));
        let map: HashMap<u32, &Process> = [&a, &b].iter().map(|p| (p.pid, *p)).collect();
        assert_eq!(ancestor_chain(&a, &map).len(), 101);
    }

    #[test]
    fn a_process_that_passes_every_filter_is_kept() {
        let mut p = process(1, None);
        p.start_time = Some(0);
        let cmd = TreeCommand {
            min_cpu: Some(0.0),
            min_mem: Some(0.0),
            status: Some(StatusFilter::Sleeping),
            min_uptime: Some(1),
            ..command()
        };
        assert!(cmd.matches_filters(&p));
        let other_status = TreeCommand {
            status: Some(StatusFilter::Running),
            ..command()
        };
        assert!(!other_status.matches_filters(&p));
    }
}
