//! Stop command - Graceful process termination (SIGTERM)
//!
//! Usage:
//!   proc stop 1234              # Stop PID 1234
//!   proc stop :3000             # Stop process on port 3000
//!   proc stop node              # Stop all node processes
//!   proc stop :3000,:8080       # Stop multiple targets
//!   proc stop :3000,1234,node   # Mixed targets (port + PID + name)

#[cfg(unix)]
use crate::core::parse_signal_name;
use crate::core::{apply_filters, parse_targets, resolve_targets_excluding_self, Process};
use crate::error::{ProcError, Result};
use crate::ui::Printer;
use clap::Args;

/// Stop process(es) gracefully with SIGTERM
#[derive(Args, Debug)]
pub struct StopCommand {
    /// Target(s): process name, PID, or :port (comma-separated for multiple)
    #[arg(required = true)]
    target: String,

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

    /// Filter by directory (defaults to current directory if no path given)
    #[arg(long = "in", short = 'i', num_args = 0..=1, default_missing_value = ".")]
    pub in_dir: Option<String>,

    /// Filter by process name
    #[arg(long = "by", short = 'b')]
    pub by_name: Option<String>,

    /// Initial signal to send instead of SIGTERM (e.g. HUP, USR1, INT)
    #[arg(long, short = 'S')]
    pub signal: Option<String>,
}

impl StopCommand {
    /// Executes the stop command, gracefully terminating matched processes.
    pub fn execute(&self) -> Result<()> {
        let printer = Printer::from_flags(self.json, self.verbose);

        // Parse comma-separated targets and resolve to processes
        // Use resolve_targets_excluding_self to avoid stopping ourselves
        let targets = parse_targets(&self.target);
        let (mut processes, not_found) = resolve_targets_excluding_self(&targets);

        // Warn about targets that weren't found
        if !not_found.is_empty() {
            printer.warning(&format!("Not found: {}", not_found.join(", ")));
        }

        // Apply --in and --by filters
        apply_filters(&mut processes, &self.in_dir, &self.by_name);

        if processes.is_empty() {
            return Err(ProcError::ProcessNotFound(self.target.clone()));
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

        // Parse custom signal if provided
        #[cfg(unix)]
        let custom_signal = if let Some(ref sig_name) = self.signal {
            Some(parse_signal_name(sig_name)?)
        } else {
            None
        };

        // Stop processes
        let mut stopped = Vec::new();
        let mut failed = Vec::new();

        for proc in &processes {
            #[cfg(unix)]
            let send_result = if let Some(signal) = custom_signal {
                proc.send_signal(signal)
            } else {
                proc.terminate()
            };
            #[cfg(not(unix))]
            let send_result = proc.terminate();

            match send_result {
                Ok(()) => {
                    // Wait for process to exit
                    let stopped_gracefully = self.wait_for_exit(proc);
                    if stopped_gracefully {
                        stopped.push(proc.clone());
                    } else {
                        // Force kill after timeout - use kill_and_wait for reliability
                        match proc.kill_and_wait() {
                            Ok(_) => stopped.push(proc.clone()),
                            Err(e) => failed.push((proc.clone(), e.to_string())),
                        }
                    }
                }
                Err(e) => failed.push((proc.clone(), e.to_string())),
            }
        }

        // Output results
        printer.print_action_result("stop", &stopped, &failed);

        Ok(())
    }

    fn wait_for_exit(&self, proc: &Process) -> bool {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(self.timeout);

        while start.elapsed() < timeout {
            if !proc.is_running() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        false
    }
}
