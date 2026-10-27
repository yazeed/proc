//! Run one action over several processes and sort the results

use super::Process;
use crate::error::Result;

/// Runs `act` on each process in order. Returns the processes it worked on and
/// the ones it failed on (with the error text), each in the order given.
///
/// This is the loop `kill`, `stop`, `freeze`, `thaw` and `orphans --kill` share;
/// taking the action as a parameter is what lets a test make it fail.
pub fn apply_to_each(
    processes: &[Process],
    mut act: impl FnMut(&Process) -> Result<()>,
) -> (Vec<Process>, Vec<(Process, String)>) {
    let mut succeeded = Vec::new();
    let mut failed = Vec::new();
    for proc in processes {
        match act(proc) {
            Ok(()) => succeeded.push(proc.clone()),
            Err(e) => failed.push((proc.clone(), e.to_string())),
        }
    }
    (succeeded, failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ProcError;

    /// Three records of this very process, told apart by the PID field
    fn three() -> Vec<Process> {
        let me = Process::find_by_pid(std::process::id())
            .unwrap()
            .expect("this test process");
        (1..=3).map(|pid| Process { pid, ..me.clone() }).collect()
    }

    #[test]
    fn sorts_each_process_by_what_the_action_did_to_it_in_order() {
        let processes = three();
        let (worked, failed) = apply_to_each(&processes, |p| {
            if p.pid == 2 {
                Err(ProcError::SignalError("no permission".to_string()))
            } else {
                Ok(())
            }
        });
        assert_eq!(worked.iter().map(|p| p.pid).collect::<Vec<_>>(), [1, 3]);
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].0.pid, 2);
        assert_eq!(failed[0].1, "Signal failed: no permission");
    }

    #[test]
    fn runs_the_action_once_per_process_even_after_a_failure() {
        let processes = three();
        let mut seen = Vec::new();
        let (worked, failed) = apply_to_each(&processes, |p| {
            seen.push(p.pid);
            Err(ProcError::ProcessGone(p.pid))
        });
        assert_eq!(seen, [1, 2, 3]);
        assert!(worked.is_empty());
        assert_eq!(failed.len(), 3);
        assert_eq!(failed[2].1, "Process 3 is no longer running");
    }
}
