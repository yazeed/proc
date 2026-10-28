//! `proc orphans` - Find orphaned processes
//!
//! Examples:
//!   proc orphans              # List orphaned processes
//!   proc orphans --in .       # Orphans in current directory
//!   proc orphans --by node    # Only those named node (--cmd: node anywhere in the command line)
//!   proc orphans --kill       # Find and kill orphans
//!   proc orphans --kill --yes # Kill orphans without confirmation

use crate::core::{apply_filters, apply_to_each, NameMatch, Process};
use crate::error::Result;
use crate::ui::{must_confirm, plural, Printer};
use clap::Args;
use dialoguer::Confirm;

/// Find orphaned processes (re-parented to PID 1: init/launchd; includes processes launchd started itself)
#[derive(Args, Debug)]
pub struct OrphansCommand {
    /// Kill found orphaned processes
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

    /// Filter by directory; takes the next word as the directory unless it starts with '-'
    /// (bare --in is the current directory, --in=DIR is explicit)
    #[arg(long = "in", short = 'i', num_args = 0..=1, default_missing_value = ".")]
    pub in_dir: Option<String>,

    /// Filter by process name
    #[arg(long = "by", short = 'b')]
    pub by_name: Option<String>,

    /// Match --by as a substring of the process name or full command line (like pkill -f)
    #[arg(long)]
    pub cmd: bool,
}

impl OrphansCommand {
    /// Executes the orphans command, finding processes whose parent is PID 1.
    pub fn execute(&self) -> Result<()> {
        self.run(Process::find_orphans()?)
    }

    /// Everything after the lookup: filter `processes` (the orphans found), then
    /// list them or, with `--kill`, kill them. Apart from `execute` it is called
    /// with processes a test made, because no process is an orphan on Windows
    /// (there is no PID 1 to be adopted by), so a Windows run can't make one.
    fn run(&self, mut processes: Vec<Process>) -> Result<()> {
        let printer = Printer::from_flags(self.json, self.verbose).wide(self.wide);

        // Apply --in and --by filters
        apply_filters(
            &mut processes,
            &self.in_dir,
            &self.by_name,
            NameMatch::from_flag(self.cmd),
        );

        // Real CPU readings for every view that shows or returns the list; only
        // `--kill --json --yes` (no dry run) goes straight to killing
        if !(self.kill && self.json && self.yes && !self.dry_run) {
            Process::measure_cpu(&mut processes);
        }

        if processes.is_empty() {
            printer.print_empty_result("orphans", "No orphaned processes found");
            return Ok(());
        }

        printer.warning(&format!(
            "Found {} orphaned process{}",
            processes.len(),
            plural(processes.len())
        ));

        if !self.kill {
            printer.print_processes_under_headline("orphans", &processes);
            return Ok(());
        }

        // Dry run: show what would be killed
        if self.dry_run {
            printer.print_dry_run_with(
                "orphans",
                &processes,
                &format!(
                    "Dry run: would kill {} orphaned process{}",
                    processes.len(),
                    plural(processes.len())
                ),
            );
            return Ok(());
        }

        // Kill: JSON prints a single action result; humans see the list first
        if !self.json {
            printer.print_processes_under_headline("orphans", &processes);
        }

        // Same rule everywhere: --yes, or ask; --json never skips it
        if must_confirm("orphans --kill", self.yes, self.json, &processes, None)? {
            let confirmed = Confirm::new()
                .with_prompt(format!(
                    "Kill {} orphaned process{}?",
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

        let (killed, failed) = apply_to_each(&processes, |proc| proc.kill_and_wait());

        printer.print_action_result_as("orphans", "kill", &killed, &failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ProcError;
    use crate::test_support::{end, idle_child, record_of};

    fn command() -> OrphansCommand {
        OrphansCommand {
            kill: false,
            dry_run: false,
            yes: false,
            json: false,
            verbose: false,
            wide: false,
            in_dir: None,
            by_name: None,
            cmd: false,
        }
    }

    #[test]
    fn no_orphans_is_not_an_error() {
        command().run(Vec::new()).unwrap();
        let json = OrphansCommand {
            json: true,
            ..command()
        };
        json.run(Vec::new()).unwrap();
    }

    #[test]
    fn a_filter_that_matches_none_of_them_leaves_nothing_to_list() {
        let mut child = idle_child();
        let orphan = record_of(&child);
        let elsewhere = tempfile::tempdir().unwrap();
        let kill = OrphansCommand {
            kill: true,
            yes: true,
            in_dir: Some(elsewhere.path().to_string_lossy().into_owned()),
            ..command()
        };
        let result = kill.run(vec![orphan]);
        let alive = child.try_wait().unwrap().is_none();
        end(&mut child);
        result.unwrap();
        assert!(alive, "a process outside --in was killed");
    }

    #[test]
    fn listing_orphans_kills_nothing() {
        let mut child = idle_child();
        let orphan = record_of(&child);
        for json in [false, true] {
            let list = OrphansCommand {
                json,
                verbose: true,
                ..command()
            };
            list.run(vec![orphan.clone()]).unwrap();
        }
        let alive = child.try_wait().unwrap().is_none();
        end(&mut child);
        assert!(alive);
    }

    #[test]
    fn a_dry_run_kills_nothing() {
        let mut child = idle_child();
        let orphan = record_of(&child);
        for json in [false, true] {
            let dry = OrphansCommand {
                kill: true,
                dry_run: true,
                json,
                ..command()
            };
            dry.run(vec![orphan.clone()]).unwrap();
        }
        let alive = child.try_wait().unwrap().is_none();
        end(&mut child);
        assert!(alive);
    }

    #[test]
    fn nobody_to_ask_is_not_a_yes() {
        let mut child = idle_child();
        let orphan = record_of(&child);
        // --json never answers the prompt, and the test has no terminal
        let kill = OrphansCommand {
            kill: true,
            json: true,
            ..command()
        };
        let result = kill.run(vec![orphan]);
        let alive = child.try_wait().unwrap().is_none();
        end(&mut child);
        assert!(
            matches!(result, Err(ProcError::ConfirmationRequired { .. })),
            "{result:?}"
        );
        assert!(alive);
    }

    #[test]
    fn kill_with_yes_ends_the_orphans() {
        for json in [false, true] {
            let mut child = idle_child();
            let orphan = record_of(&child);
            let kill = OrphansCommand {
                kill: true,
                yes: true,
                json,
                ..command()
            };
            // `run` returns Ok only once the kill was seen to have worked, so
            // waiting for the child returns at once, with a failure status:
            // it did not exit by itself (a sleep of five minutes)
            kill.run(vec![orphan]).unwrap();
            let status = child.wait().unwrap();
            assert!(
                !status.success(),
                "--kill --yes did not end the orphan (json: {json})"
            );
        }
    }
}
