//! Tree command - Show process tree
//!
//! Usage:
//!   proc tree              # Full process tree
//!   proc tree node         # Tree for node processes
//!   proc tree :3000        # Tree for process on port 3000
//!   proc tree 1234         # Tree for PID 1234
//!   proc tree --min-cpu 10 # Only processes using >10% CPU
//!   proc tree 1234 -a      # Show ancestry (path UP to root)

use crate::core::{parse_target, resolve_target, Process, ProcessStatus, TargetType};
use crate::error::Result;
use crate::ui::{OutputFormat, Printer};
use clap::Args;
use colored::*;
use serde::Serialize;
use std::collections::HashMap;

/// Show process tree
#[derive(Args, Debug)]
pub struct TreeCommand {
    /// Target: process name, :port, or PID (shows full tree if omitted)
    target: Option<String>,

    /// Show ancestry (path UP to root) instead of descendants
    #[arg(long, short)]
    ancestors: bool,

    /// Output as JSON
    #[arg(long, short)]
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
    #[arg(long)]
    status: Option<String>,
}

impl TreeCommand {
    pub fn execute(&self) -> Result<()> {
        let format = if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };
        let printer = Printer::new(format, false);

        // Get all processes
        let all_processes = Process::find_all()?;

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
                    let resolved = resolve_target(target)?;
                    if resolved.is_empty() {
                        printer.warning(&format!("No process found for '{}'", target));
                        return Ok(());
                    }
                    // Find matching processes in all_processes
                    let pids: Vec<u32> = resolved.iter().map(|p| p.pid).collect();
                    all_processes
                        .iter()
                        .filter(|p| pids.contains(&p.pid))
                        .collect()
                }
                TargetType::Name(ref pattern) => {
                    // For name, do pattern matching
                    let pattern_lower = pattern.to_lowercase();
                    all_processes
                        .iter()
                        .filter(|p| {
                            p.name.to_lowercase().contains(&pattern_lower)
                                || p.command
                                    .as_ref()
                                    .map(|c| c.to_lowercase().contains(&pattern_lower))
                                    .unwrap_or(false)
                        })
                        .collect()
                }
            }
        } else {
            Vec::new() // Will show full tree
        };

        // Apply resource filters if specified
        let matches_filters = |p: &Process| -> bool {
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
            if let Some(ref status) = self.status {
                let status_match = match status.to_lowercase().as_str() {
                    "running" => matches!(p.status, ProcessStatus::Running),
                    "sleeping" | "sleep" => matches!(p.status, ProcessStatus::Sleeping),
                    "stopped" | "stop" => matches!(p.status, ProcessStatus::Stopped),
                    "zombie" => matches!(p.status, ProcessStatus::Zombie),
                    _ => true,
                };
                if !status_match {
                    return false;
                }
            }
            true
        };

        // Apply filters to target processes or find filtered roots
        let has_filters = self.min_cpu.is_some() || self.min_mem.is_some() || self.status.is_some();

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
                if filtered.len() == 1 { "" } else { "es" }
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
            let status_indicator = match proc.status {
                crate::core::ProcessStatus::Running => "●".green(),
                crate::core::ProcessStatus::Sleeping => "○".blue(),
                crate::core::ProcessStatus::Stopped => "◐".yellow(),
                crate::core::ProcessStatus::Zombie => "✗".red(),
                _ => "?".white(),
            };

            println!(
                "{}{}{} {} [{}] {:.1}% {:.1}MB",
                prefix.bright_black(),
                connector.bright_black(),
                status_indicator,
                proc.name.white().bold(),
                proc.pid.to_string().cyan(),
                proc.cpu_percent,
                proc.memory_mb
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
            status: format!("{:?}", proc.status),
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
                let pattern_lower = pattern.to_lowercase();
                pid_map
                    .values()
                    .filter(|p| {
                        p.name.to_lowercase().contains(&pattern_lower)
                            || p.command
                                .as_ref()
                                .map(|c| c.to_lowercase().contains(&pattern_lower))
                                .unwrap_or(false)
                    })
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
                action: "ancestry",
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
        // Build the ancestor chain (from target up to root)
        let mut chain: Vec<&Process> = Vec::new();
        let mut current_pid = Some(target.pid);

        while let Some(pid) = current_pid {
            if let Some(proc) = pid_map.get(&pid) {
                chain.push(proc);
                current_pid = proc.parent_pid;
                // Prevent infinite loops
                if chain.len() > 100 {
                    break;
                }
            } else {
                break;
            }
        }

        // Reverse to print from root to target
        chain.reverse();

        // Print the chain
        for (i, proc) in chain.iter().enumerate() {
            let is_target = proc.pid == target.pid;
            let indent = "    ".repeat(i);
            let connector = if i == 0 { "" } else { "└── " };

            let status_indicator = match proc.status {
                ProcessStatus::Running => "●".green(),
                ProcessStatus::Sleeping => "○".blue(),
                ProcessStatus::Stopped => "◐".yellow(),
                ProcessStatus::Zombie => "✗".red(),
                _ => "?".white(),
            };

            if is_target {
                // Highlight the target
                println!(
                    "{}{}{} {} [{}] {:.1}% {:.1}MB  {}",
                    indent.bright_black(),
                    connector.bright_black(),
                    status_indicator,
                    proc.name.cyan().bold(),
                    proc.pid.to_string().cyan().bold(),
                    proc.cpu_percent,
                    proc.memory_mb,
                    "← target".yellow()
                );
            } else {
                println!(
                    "{}{}{} {} [{}] {:.1}% {:.1}MB",
                    indent.bright_black(),
                    connector.bright_black(),
                    status_indicator,
                    proc.name.white(),
                    proc.pid.to_string().cyan(),
                    proc.cpu_percent,
                    proc.memory_mb
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
        let mut chain: Vec<ProcessInfo> = Vec::new();
        let mut current_pid = Some(target.pid);

        while let Some(pid) = current_pid {
            if let Some(proc) = pid_map.get(&pid) {
                chain.push(ProcessInfo {
                    pid: proc.pid,
                    name: proc.name.clone(),
                    cpu_percent: proc.cpu_percent,
                    memory_mb: proc.memory_mb,
                    status: format!("{:?}", proc.status),
                });
                current_pid = proc.parent_pid;
                if chain.len() > 100 {
                    break;
                }
            } else {
                break;
            }
        }

        chain.reverse();

        AncestryNode {
            target_pid: target.pid,
            target_name: target.name.clone(),
            depth: chain.len(),
            chain,
        }
    }
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
    status: String,
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
    status: String,
    children: Vec<TreeNode>,
}
