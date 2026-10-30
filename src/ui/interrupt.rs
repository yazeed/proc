//! Putting the terminal back when a signal ends proc.
//!
//! `watch` turns the terminal's raw mode on, switches to the alternate screen
//! and hides the cursor; a confirmation prompt hides the cursor and turns off
//! echo and line editing while it waits for a key. A normal end puts all of it
//! back. A signal does not run any of that: the process just dies, and the
//! shell is left typing nothing back and showing no cursor until `stty sane`.
//!
//! While a [`Guard`] is alive, SIGINT, SIGTERM, SIGHUP and SIGQUIT (on Windows
//! Ctrl-C, Ctrl-Break and a closing console) are caught, the terminal is put
//! back, and proc ends by that same signal, so a shell reports 128 plus the
//! signal number (130 for Ctrl-C, 143 for SIGTERM, 129 for SIGHUP, 131 for
//! SIGQUIT) and knows a signal ended it: Ctrl-C stops a script's loop. Windows
//! has no signals, so there proc exits with 130. A signal is how a person ends
//! a program, so it is not one of proc's own exit codes, and nothing is printed.
//!
//! The signal is not handled in the signal handler, which may not take a lock
//! or allocate: `signal-hook` only records it, and a thread of ours that waits
//! for it does the work. The thread is started by the first guard and then
//! stays, so a signal after a guard is gone also ends proc this way (nothing
//! is left to put back by then). While a guard is alive the calling thread
//! blocks these signals, so they are always taken by that thread and never
//! interrupt the one that is reading a key.
//!
//! Ctrl+C typed into a prompt is not a signal (the terminal is in raw mode, so
//! the key arrives as a character and the prompt reports it as an interrupted
//! read): [`ctrl_c`] ends proc the same way.

use crate::ui::out;
use crossterm::{cursor, execute, terminal};
use std::io::Write;
use std::sync::{Mutex, MutexGuard, Once};

/// What the terminal state to put back is
enum Restore {
    /// `watch`: raw mode, the alternate screen and the hidden cursor
    Screen,
    /// A prompt: the cursor, and the line mode that was set before it asked
    Prompt(Saved),
}

/// The terminal's line mode (echo, line editing) before a prompt changed it
#[cfg(unix)]
type Saved = Option<libc::termios>;
#[cfg(not(unix))]
struct Saved;

/// What to put back if a signal comes now; none while nothing is changed
static NEEDS: Mutex<Option<Restore>> = Mutex::new(None);

fn needs() -> MutexGuard<'static, Option<Restore>> {
    // The state is plain data; a panic while holding the lock leaves it right
    NEEDS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Keeps the terminal state in `NEEDS` for as long as it is alive
pub struct Guard(());

impl Guard {
    fn arm(restore: Restore) -> Self {
        static LISTENING: Once = Once::new();
        // The thread starts before this one blocks the signals, so it does not
        // inherit the block
        LISTENING.call_once(listen);
        *needs() = Some(restore);
        block_signals(true);
        Guard(())
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        block_signals(false);
        *needs() = None;
    }
}

/// From here until the guard is dropped, a signal puts the screen back: raw
/// mode off, cursor shown, normal screen
pub fn protect_screen() -> Guard {
    Guard::arm(Restore::Screen)
}

/// From here until the guard is dropped, a signal shows the cursor and puts
/// the line mode back to what it is now
pub fn protect_prompt() -> Guard {
    Guard::arm(Restore::Prompt(save_mode()))
}

/// Put the terminal back the way it was before `watch`: raw mode off, cursor
/// shown, normal screen. Every step is tried, whatever an earlier one did.
pub fn restore_screen(out: &mut impl Write) {
    let _ = terminal::disable_raw_mode();
    let _ = execute!(out, cursor::Show, terminal::LeaveAlternateScreen);
}

/// Show the cursor a prompt hid and put the line mode back
fn restore_prompt(out: &mut impl Write, saved: &Saved) {
    let _ = execute!(out, cursor::Show);
    put_back_mode(saved);
}

#[cfg(unix)]
fn save_mode() -> Saved {
    let mut mode = std::mem::MaybeUninit::<libc::termios>::uninit();
    // SAFETY: tcgetattr fills the struct when it returns 0
    let read = unsafe { libc::tcgetattr(libc::STDIN_FILENO, mode.as_mut_ptr()) } == 0;
    // SAFETY: initialised, because tcgetattr returned 0
    read.then(|| unsafe { mode.assume_init() })
}

#[cfg(unix)]
fn put_back_mode(saved: &Saved) {
    if let Some(mode) = saved {
        // SAFETY: a valid termios, read from this same descriptor earlier
        unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, mode) };
    }
}

#[cfg(not(unix))]
fn save_mode() -> Saved {
    Saved
}

#[cfg(not(unix))]
fn put_back_mode(_: &Saved) {}

/// Put back whatever the armed guard says is changed
fn put_right() {
    match needs().as_ref() {
        Some(Restore::Screen) => restore_screen(&mut out::stdout_unrecorded()),
        Some(Restore::Prompt(saved)) => restore_prompt(&mut out::stderr_unrecorded(), saved),
        None => {}
    }
}

/// Block (or unblock) the signals we listen for on the calling thread, so that
/// another thread takes them: this one may be sitting in a read that a signal
/// would cut short, and then fail and end proc before the signal's own thread
/// had ended it with the right status.
#[cfg(unix)]
fn block_signals(block: bool) {
    // SAFETY: an empty set filled with four valid signal numbers, then applied
    // to this thread only
    unsafe {
        let mut set = std::mem::MaybeUninit::<libc::sigset_t>::uninit();
        libc::sigemptyset(set.as_mut_ptr());
        for signal in LISTENED {
            libc::sigaddset(set.as_mut_ptr(), signal);
        }
        let how = if block {
            libc::SIG_BLOCK
        } else {
            libc::SIG_UNBLOCK
        };
        libc::pthread_sigmask(how, set.as_ptr(), std::ptr::null_mut());
    }
}

#[cfg(not(unix))]
fn block_signals(_: bool) {}

/// Start `command`'s child with no signal blocked. While a guard is alive this
/// thread blocks the signals that end proc, and a child inherits the mask (the
/// standard library keeps it), so an `lsof` that `watch` runs on a refresh
/// could not be stopped by Ctrl-C, SIGTERM, SIGHUP or SIGQUIT.
#[cfg(unix)]
pub fn unblocked_in_child(command: &mut std::process::Command) -> &mut std::process::Command {
    use std::os::unix::process::CommandExt;
    // SAFETY: the hook runs in the child between fork and exec, and calls only
    // sigemptyset and pthread_sigmask, which are async-signal-safe
    unsafe { command.pre_exec(unblock_all) }
}

/// Block no signal on the calling thread
#[cfg(unix)]
fn unblock_all() -> std::io::Result<()> {
    // SAFETY: an empty set, applied to this thread only
    unsafe {
        let mut set = std::mem::MaybeUninit::<libc::sigset_t>::uninit();
        libc::sigemptyset(set.as_mut_ptr());
        libc::pthread_sigmask(libc::SIG_SETMASK, set.as_ptr(), std::ptr::null_mut());
    }
    Ok(())
}

/// The signals that end proc, here and in `listen`
#[cfg(unix)]
const LISTENED: [i32; 4] = [
    signal_hook::consts::SIGINT,
    signal_hook::consts::SIGTERM,
    signal_hook::consts::SIGHUP,
    signal_hook::consts::SIGQUIT,
];

/// End proc as Ctrl+C does, status 130, after putting the terminal right. For a
/// prompt that reports Ctrl+C as a key instead of the signal.
pub fn ctrl_c() {
    serve(std::iter::once(2), exit_with);
}

fn exit_with(code: i32) {
    // End by the signal itself, as a program without this handler would. A
    // shell then knows the signal ended proc: Ctrl-C stops a script's loop,
    // which an exit status of 130 does not, though it reads the same. `exit`
    // is for Windows, which has no signals.
    #[cfg(unix)]
    {
        write_coverage();
        let _ = signal_hook::low_level::emulate_default_handler(code - 128);
    }
    std::process::exit(code)
}

/// A process that a signal ends writes no coverage profile, so a coverage
/// build writes it first: the tests that end proc by a signal are the only
/// ones that run this part of it
#[cfg(all(unix, coverage))]
fn write_coverage() {
    extern "C" {
        fn __llvm_profile_write_file() -> libc::c_int;
    }
    // SAFETY: the profiler runtime that every coverage build links defines it
    unsafe { __llvm_profile_write_file() };
}

#[cfg(all(unix, not(coverage)))]
fn write_coverage() {}

/// For each signal that arrives: put the terminal right, then end proc by it.
/// `end` gets 128 plus the signal's number and is [`exit_with`]; a test passes
/// something that does not exit.
fn serve(signals: impl Iterator<Item = i32>, end: impl Fn(i32)) {
    for signal in signals {
        put_right();
        end(128 + signal);
    }
}

#[cfg(unix)]
fn listen() {
    // If the signals cannot be watched there is nothing to be done about it:
    // proc goes on as it did before this existed
    let _ = signal_hook::iterator::Signals::new(LISTENED)
        .map(|mut signals| std::thread::spawn(move || serve(signals.forever(), exit_with)));
}

#[cfg(windows)]
fn listen() {
    let _ = ctrlc::set_handler(ctrl_c);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn restoring_the_screen_shows_the_cursor_and_leaves_the_alternate_screen() {
        let mut out = Vec::new();
        restore_screen(&mut out);
        assert_eq!(String::from_utf8(out).unwrap(), "\x1b[?25h\x1b[?1049l");
    }

    #[test]
    fn restoring_a_prompt_shows_the_cursor() {
        let mut out = Vec::new();
        restore_prompt(&mut out, &save_mode());
        assert_eq!(String::from_utf8(out).unwrap(), "\x1b[?25h");
    }

    // One test for both, because both use the one static
    #[test]
    fn a_signal_ends_proc_with_128_plus_its_number_and_a_guard_clears_the_state() {
        let ended = RefCell::new(Vec::new());
        // SIGHUP, SIGINT, SIGQUIT, SIGTERM
        serve([1, 2, 3, 15].into_iter(), |code| {
            ended.borrow_mut().push(code)
        });
        assert_eq!(*ended.borrow(), [129, 130, 131, 143]);

        // Nothing is armed through `Guard::arm` here, which would start
        // catching signals in the test run
        *needs() = Some(Restore::Prompt(save_mode()));
        assert!(needs().is_some());
        drop(Guard(()));
        assert!(needs().is_none());
    }

    // A child started while a guard blocks the signals can still be ended by
    // them. `sleep` keeps the default action, so a blocked SIGTERM would leave
    // it running for the whole minute.
    #[cfg(unix)]
    #[test]
    fn a_child_started_while_the_signals_are_blocked_can_still_be_ended() {
        use std::os::unix::process::ExitStatusExt;
        block_signals(true);
        let child = unblocked_in_child(&mut std::process::Command::new("sleep"))
            .arg("60")
            .spawn();
        // The parent half of the test leaves this thread as it found it
        let _ = unblock_all();
        let mut child = child.unwrap();
        unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
        let start = std::time::Instant::now();
        let status = child.wait().unwrap();
        assert_eq!(status.signal(), Some(libc::SIGTERM));
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
    }
}
