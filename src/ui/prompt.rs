//! The one place proc asks a yes/no question.
//!
//! Every command that changes processes asks through [`ask`]: `kill`, `stop`,
//! `freeze`, `thaw`, `free`, `unstick`, `stuck --kill` and `orphans --kill`.
//! (`clippy.toml` bans dialoguer's `interact` methods everywhere else.)
//!
//! A person who approves something must be able to see it. The list of what
//! would change used to go to stdout while the question went to stderr, so
//! with stdout in a file, or a pipe whose reader had left (`proc kill node |
//! head -n1`), the question came with no list and `y` was a blind yes. Now:
//!
//! - **stdout is a terminal**: the list goes to stdout, as it always did.
//! - **stdout is anything else** (a file, a pipe): the list goes to stderr,
//!   right above the question, which is on stderr and so is a terminal here
//!   (see [`must_confirm`](crate::ui::must_confirm)). The person sees what they
//!   approve and stdout stays what the command's output is.
//! - **the list could not be written** where it was meant to go: nothing is
//!   asked and nothing is changed. It is exit 4, `needs_confirmation`, like
//!   any other case where nobody can be asked.
//!
//! The question also restores the terminal when a signal ends proc while it
//! is waiting (see [`interrupt`]), and shows the cursor again when it is over
//! however it ended.

use crate::core::Process;
use crate::error::{ProcError, Result};
use crate::ui::{interrupt, out};
use crossterm::{cursor, execute};
use dialoguer::Confirm;

/// Show `show`, the list of `processes` that `action` is about to change, and
/// ask `question`. `Ok(true)` is a yes and `Ok(false)` a no.
///
/// `show` prints the list with the usual output macros; this decides which
/// stream they reach. When the list cannot be shown the result is the error
/// `ConfirmationRequired`, carrying `processes` and `not_found` as every
/// refusal does.
pub fn ask(
    action: &str,
    processes: &[Process],
    not_found: Option<&[String]>,
    question: &str,
    show: impl FnOnce(),
) -> Result<bool> {
    let shown = list_shown(out::stdout_is_terminal(), show);
    answer_if_shown(shown, action, processes, not_found, || interact(question))
}

/// Run `show` where the person will see it, and say whether it got there
fn list_shown(stdout_is_terminal: bool, show: impl FnOnce()) -> bool {
    if stdout_is_terminal {
        show();
        out::stdout_usable()
    } else {
        out::on_stderr(show);
        out::stderr_usable()
    }
}

/// Ask only if the list was shown; refuse otherwise
fn answer_if_shown(
    shown: bool,
    action: &str,
    processes: &[Process],
    not_found: Option<&[String]>,
    answer: impl FnOnce() -> Result<bool>,
) -> Result<bool> {
    if !shown {
        return Err(ProcError::ConfirmationRequired {
            message: format!(
                "{action} asks before changing processes, and the list of what it would \
                 change could not be shown; pass --yes (or --dry-run to preview)"
            ),
            processes: processes.to_vec(),
            not_found: not_found.map(<[String]>::to_vec),
        });
    }
    answer()
}

/// The question itself: the one call to dialoguer
#[allow(clippy::disallowed_methods)]
fn interact(question: &str) -> Result<bool> {
    let _protected = interrupt::protect_prompt();
    let answer = Confirm::new()
        .with_prompt(question)
        .default(false)
        .interact();
    // Ctrl+C typed at the prompt is an interrupted read, not a signal
    if let Err(dialoguer::Error::IO(error)) = &answer {
        if error.kind() == std::io::ErrorKind::Interrupted {
            interrupt::ctrl_c();
        }
    }
    // dialoguer hides the cursor while it waits; a prompt that ended in an
    // error leaves it hidden
    let _ = execute!(out::stderr_unrecorded(), cursor::Show);
    Ok(answer?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn the_list_is_shown_on_either_stream_and_the_question_waits_for_it() {
        for stdout_is_terminal in [true, false] {
            let shown = Cell::new(0);
            assert!(list_shown(stdout_is_terminal, || shown.set(shown.get() + 1)));
            assert_eq!(shown.get(), 1);
        }
    }

    #[test]
    fn a_list_that_was_not_shown_is_a_refusal_with_what_would_have_changed() {
        let missing = vec!["ghost".to_string()];
        let refused = answer_if_shown(false, "kill", &[], Some(&missing), || panic!("asked"))
            .expect_err("nothing is asked when the list was not shown");
        assert_eq!(crate::error::ExitCode::from(&refused) as i32, 4);
        assert_eq!(refused.reason(), Some("needs_confirmation"));
        assert!(
            refused.to_string().contains("could not be shown"),
            "{refused}"
        );
        assert!(format!("{refused:?}").contains("ghost"), "{refused:?}");
    }

    #[test]
    fn a_shown_list_leaves_the_answer_to_the_person() {
        assert!(answer_if_shown(true, "kill", &[], None, || Ok(true)).unwrap());
        assert!(!answer_if_shown(true, "kill", &[], None, || Ok(false)).unwrap());
    }
}
