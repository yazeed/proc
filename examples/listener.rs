//! A process that owns a listening port, for tests to aim proc at.
//!
//! Binds `127.0.0.1:0`, prints `PORT <n>` on stdout and flushes, then sleeps
//! until it is killed. Flags, for tests that need a process that behaves
//! differently:
//! - `--ignore-term` ignores SIGTERM on Unix (a no-op on Windows), so a
//!   graceful stop cannot move it.
//! - `--busy` spins on a CPU core instead of sleeping, so proc reads it as stuck.
//! - `--fork` (Unix only) forks after binding and prints `CHILD <pid>`: the
//!   child keeps the listening socket after the parent is killed, so the port
//!   stays in use.

use std::io::Write;
use std::net::TcpListener;
use std::time::Duration;

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
    loop {
        if busy {
            std::hint::black_box(0u64);
        } else {
            std::thread::sleep(Duration::from_secs(60));
        }
        let _ = &listener;
    }
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
