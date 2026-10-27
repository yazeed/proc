//! `proc stuck` - Find stuck/hung processes
//!
//! A process counts as stuck when it has been running longer than --timeout and,
//! in two readings a second apart, is using more than 50% CPU or is blocked in
//! uninterruptible sleep. A long build at full CPU looks the same as a loop
//! that never ends, so check the list before --kill.
//!
//! Examples:
//!   proc stuck              # Running over 5 minutes, and busy or blocked for 2 seconds
//!   proc stuck --timeout 60 # The same, for processes running over 1 minute
//!   proc stuck --kill       # Find and kill stuck processes

use crate::core::{apply_filters, Process};
use crate::error::Result;
use crate::ui::{must_confirm, plural, Printer};
use clap::Args;
use dialoguer::Confirm;
use std::time::Duration;

/// Find stuck/hung processes
#[derive(Args, Debug)]
pub struct StuckCommand {
    /// Only processes running longer than this many seconds (stuck: over 50% CPU or blocked, for 2 seconds)
    #[arg(long, short = 't', default_value = "300")]
    pub timeout: u64,

    /// Kill found stuck processes
    #[arg(long, short = 'k')]
    pub kill: bool,

    /// Show what would be killed without actually killing
    #[arg(long)]
    pub dry_run: bool,

    /// Skip confirmation when killing
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Output as JSON
    #[arg(long, short = 'j')]
    pub json: bool,

    /// Show verbose output
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Print full directories and arguments instead of fitting the terminal width
    #[arg(long, short = 'w')]
    pub wide: bool,

    /// Filter by directory (defaults to current directory if no path given)
    #[arg(long = "in", short = 'i', num_args = 0..=1, default_missing_value = ".")]
    pub in_dir: Option<String>,

    /// Filter by process name
    #[arg(long = "by", short = 'b')]
    pub by_name: Option<String>,
}

impl StuckCommand {
    /// Executes the stuck command, finding long-running processes using high CPU.
    pub fn execute(&self) -> Result<()> {
        let printer = Printer::from_flags(self.json, self.verbose).wide(self.wide);

        let timeout = Duration::from_secs(self.timeout);
        let mut processes = Process::find_stuck(timeout)?;

        // Apply --in and --by filters
        apply_filters(&mut processes, &self.in_dir, &self.by_name);

        if processes.is_empty() {
            printer.print_empty_result(
                "stuck",
                &format!("No stuck processes found (threshold: {}s)", self.timeout),
            );
            return Ok(());
        }

        printer.warning(&format!(
            "Found {} potentially stuck process{}",
            processes.len(),
            plural(processes.len())
        ));

        if !self.kill {
            printer.print_processes_under_headline("stuck", &processes);
            return Ok(());
        }

        // Dry run: show what would be killed
        if self.dry_run {
            printer.print_dry_run_with(
                "stuck",
                &processes,
                &format!(
                    "Dry run: would kill {} stuck process{}",
                    processes.len(),
                    plural(processes.len())
                ),
            );
            return Ok(());
        }

        // Kill: JSON prints a single action result; humans see the list first
        if !self.json {
            printer.print_processes_under_headline("stuck", &processes);
        }

        // Same rule everywhere: --yes, or ask; --json never skips it
        if must_confirm("stuck --kill", self.yes, self.json, &processes)? {
            let confirmed = Confirm::new()
                .with_prompt(format!(
                    "Kill {} stuck process{}?",
                    processes.len(),
                    plural(processes.len())
                ))
                .default(false)
                .interact()?;

            if !confirmed {
                printer.warning("Cancelled");
                return Ok(());
            }
        }

        // Use kill_and_wait to ensure stuck processes are actually terminated
        let (killed, failed) = kill_each(processes, Process::kill_and_wait);

        printer.print_action_result_as("stuck", "kill", &killed, &failed)
    }
}

/// Kill each process with `kill`, sorting them into those that ended and those
/// that did not, with the reason
fn kill_each(
    processes: Vec<Process>,
    kill: impl Fn(&Process) -> Result<()>,
) -> (Vec<Process>, Vec<(Process, String)>) {
    let mut killed = Vec::new();
    let mut failed = Vec::new();
    for proc in processes {
        match kill(&proc) {
            Ok(_) => killed.push(proc),
            Err(e) => failed.push((proc, e.to_string())),
        }
    }
    (killed, failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ProcessStatus;
    use crate::error::ProcError;

    fn process(pid: u32) -> Process {
        Process {
            pid,
            name: "x".to_string(),
            exe_path: None,
            cwd: None,
            command: None,
            cpu_percent: 0.0,
            memory_mb: 0.0,
            status: ProcessStatus::Running,
            user: None,
            parent_pid: None,
            start_time: None,
            uptime_secs: None,
        }
    }

    #[test]
    fn a_process_that_cannot_be_killed_is_reported_with_its_reason() {
        let (killed, failed) = kill_each(vec![process(1), process(2)], |p| {
            if p.pid == 2 {
                Err(ProcError::SignalError("still running".to_string()))
            } else {
                Ok(())
            }
        });
        assert_eq!(killed.len(), 1);
        assert_eq!(killed[0].pid, 1);
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].0.pid, 2);
        assert!(failed[0].1.contains("still running"), "{}", failed[0].1);
    }
}
