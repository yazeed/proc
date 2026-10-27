//! A process that owns a listening port, for tests to aim proc at.
//!
//! Binds `127.0.0.1:0`, prints `PORT <n>` on stdout and flushes, then sleeps
//! until it is killed, or for two minutes at most. Flags, for tests that need a process that behaves
//! differently:
//! - `--ignore-term` ignores SIGTERM on Unix (a no-op on Windows), so a
//!   graceful stop cannot move it.
//! - `--busy` spins on a CPU core instead of sleeping, so proc reads it as stuck.
//! - `--fork` (Unix only) forks after binding and prints `CHILD <pid>`: the
//!   child keeps the listening socket after the parent is killed, so the port
//!   stays in use.
//!
//! Whatever the flags, the process (and a forked copy) exits by itself after
//! two minutes: long enough for any test, short enough that a test binary
//! killed with SIGKILL, which runs no cleanup, cannot leave a spinner behind.

use std::io::Write;
use std::net::TcpListener;
use std::time::{Duration, Instant};

/// How long the process lives if nothing kills it: far longer than any test
/// needs, short enough that a test binary killed with SIGKILL (which runs no
/// Drop) cannot leave a spinning or forked copy behind for long.
const LIFETIME: Duration = Duration::from_secs(120);

fn main() {
    let flag = |name: &str| std::env::args().any(|a| a == name);
    if flag("--ignore-term") {
        ignore_term();
    }

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1:0");
    let port = listener.local_addr().expect("local addr").port();
    println!("PORT {port}");
    std::io::stdout().flush().expect("flush stdout");

    if flag("--fork") {
        fork_sharing_the_socket();
    }

    // Keep the socket open for as long as we live
    let busy = flag("--busy");
    let deadline = Instant::now() + LIFETIME;
    while Instant::now() < deadline {
        if busy {
            std::hint::black_box(0u64);
        } else {
            std::thread::sleep(Duration::from_millis(500));
        }
    }
    drop(listener);
}

#[cfg(unix)]
fn ignore_term() {
    // SAFETY: installing SIG_IGN for SIGTERM is async-signal-safe and has no
    // handler state to get wrong.
    unsafe {
        libc::signal(libc::SIGTERM, libc::SIG_IGN);
    }
}

#[cfg(not(unix))]
fn ignore_term() {}

/// Fork; the parent prints the child's pid and carries on. Both processes hold
/// the listening socket.
#[cfg(unix)]
fn fork_sharing_the_socket() {
    // SAFETY: the process is single-threaded here, so fork is safe, and the
    // child only goes on to sleep.
    let pid = unsafe { libc::fork() };
    assert!(pid >= 0, "fork failed");
    if pid > 0 {
        println!("CHILD {pid}");
        std::io::stdout().flush().expect("flush stdout");
    }
}

#[cfg(not(unix))]
fn fork_sharing_the_socket() {}
