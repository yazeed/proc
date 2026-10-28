//! Stop command - Graceful process termination (SIGTERM)
//!
//! Usage:
//!   proc stop 1234              # Stop PID 1234
//!   proc stop :3000             # Stop process on port 3000
//!   proc stop node              # Stop the processes named node
//!   proc stop node --cmd        # ... or with node anywhere in their command line
//!   proc stop 1234 5678         # Stop several targets: separate arguments
//!   proc stop :3000 :8080 node  # Mixed targets (port + port + name)
//!
//! Commas still work (`proc stop :3000,:8080`) but are deprecated.

#[cfg(unix)]
use crate::core::parse_signal_name;
use crate::core::{
    apply_filters, apply_to_each, flatten_targets, resolve_targets_excluding_self, NameMatch,
    Process,
};
use crate::error::{ProcError, Result};
use crate::ui::Printer;
use clap::Args;

/// Stop process(es) gracefully with SIGTERM
#[derive(Args, Debug)]
pub struct StopCommand {
    /// Target(s): process name, PID, or :port (separate arguments for several)
    #[arg(required = true, num_args = 1.., value_parser = crate::core::validate_target_arg)]
    target: Vec<String>,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    yes: bool,

    /// Show what would be stopped without actually stopping
    #[arg(long)]
    dry_run: bool,

    /// Output as JSON
    #[arg(long, short = 'j')]
    json: bool,

    /// Show verbose output
    #[arg(long, short = 'v')]
    verbose: bool,

    /// Timeout in seconds to wait before force kill
    #[arg(long, short, default_value = "10")]
    timeout: u64,

    /// Filter by directory; takes the next word as the directory unless it starts with '-'
    /// (bare --in is the current directory, --in=DIR is explicit)
    #[arg(long = "in", short = 'i', num_args = 0..=1, default_missing_value = ".")]
    pub in_dir: Option<String>,

    /// Filter by process name
    #[arg(long = "by", short = 'b')]
    pub by_name: Option<String>,

    /// Match name targets and --by as a substring of the process name or full command line (like pkill -f)
    #[arg(long)]
    pub cmd: bool,

    /// Initial signal to send instead of SIGTERM (e.g. HUP, USR1, INT)
    #[arg(long, short = 'S')]
    pub signal: Option<String>,
}

impl StopCommand {
    /// Executes the stop command, gracefully terminating matched processes.
    pub fn execute(&self) -> Result<()> {
        let printer = Printer::from_flags(self.json, self.verbose);

        // Validate --signal first, so a bad name fails the same way with or
        // without --dry-run, and before the confirmation prompt
        #[cfg(unix)]
        let custom_signal = if let Some(ref sig_name) = self.signal {
            Some(parse_signal_name(sig_name)?)
        } else {
            None
        };
        // Windows has no signals: say so rather than quietly sending a normal stop
        #[cfg(not(unix))]
        if self.signal.is_some() {
            return Err(ProcError::InvalidInput(
                "--signal is not supported on Windows (proc stop asks the process to exit)"
                    .to_string(),
            ));
        }

        // Split each value on commas and resolve to processes
        // Use resolve_targets_excluding_self to avoid stopping ourselves
        let targets = flatten_targets(&self.target);
        let mode = NameMatch::from_flag(self.cmd);
        let resolved = resolve_targets_excluding_self(&targets, mode);

        // Warn about targets that weren't found
        if let Some(warning) = resolved.warning() {
            printer.warning(&warning);
        }
        // the JSON results list the targets that matched nothing
        let printer = printer.with_not_found(&resolved.not_found);
        let mut processes = resolved.processes;

        // Apply --in and --by filters
        apply_filters(&mut processes, &self.in_dir, &self.by_name, mode);

        // Real CPU readings whenever the list is shown or returned (dry run, the
        // confirmation prompt, or the error for a missing `--yes`); only `--yes`
        // acts at once, without the wait
        if self.dry_run || !self.yes {
            Process::measure_cpu(&mut processes);
        }

        if processes.is_empty() {
            return Err(ProcError::ProcessNotFound(self.target.join(" ")));
        }

        // Dry run: just show what would be stopped
        if self.dry_run {
            printer.print_dry_run("stop", &processes);
            return Ok(());
        }

        // Confirm if not --yes
        if !printer.ask_confirm("stop", &processes, self.yes)? {
            return Ok(());
        }

        // Ask every process first, then wait for them together, so several
        // processes share one --timeout instead of taking one each
        let (signalled, mut failed) = apply_to_each(&processes, |proc| {
            #[cfg(unix)]
            if let Some(signal) = custom_signal {
                return proc.send_signal(signal);
            }
            match proc.terminate() {
                // Windows can't ask a console program to close (taskkill
                // without /F needs a window); it still gets stopped, by the
                // force kill after --timeout
                Err(_) if cfg!(not(unix)) && proc.is_running() => Ok(()),
                result => result,
            }
        });

        let outlived = self.wait_for_exit(&signalled);

        // Force kill whatever outlived the timeout
        let (stopped, failed_to_kill) = apply_to_each(&signalled, |proc| {
            if outlived.contains(&proc.pid) {
                proc.kill_and_wait()
            } else {
                Ok(())
            }
        });
        failed.extend(failed_to_kill);

        // Output results
        printer.print_action_result("stop", &stopped, &failed)
    }

    /// Wait up to --timeout for the processes to exit; returns the PIDs still running.
    fn wait_for_exit(&self, processes: &[Process]) -> Vec<u32> {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(self.timeout);
        let mut running: Vec<&Process> = processes.iter().collect();

        loop {
            running.retain(|p| p.is_running());
            if running.is_empty() || start.elapsed() >= timeout {
                return running.iter().map(|p| p.pid).collect();
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}
