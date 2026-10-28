//! A process that owns a listening port, for tests to aim proc at.
//!
//! Binds `127.0.0.1:0`, prints `PORT <n>` on stdout and flushes, then sleeps
//! until it is killed, or for two minutes at most. Flags, for tests that need a process that behaves
//! differently:
//! - `--ignore-term` ignores SIGTERM on Unix (a no-op on Windows), so a
//!   graceful stop cannot move it.
//! - `--busy` spins on a CPU core instead of sleeping, so proc reads it as stuck.
//! - `--respawn` forks after binding (on Windows, which cannot fork, it starts
//!   a copy of itself with `--takeover <port>`) and prints `CHILD <pid>`. The
//!   child holds no socket and binds the port itself the moment it is free, so
//!   when the parent is killed the port is taken again, by a process that was
//!   not listening when it was looked up.
//! - `--takeover <port>` (what the Windows `--respawn` runs): wait for the port
//!   to be free, bind it, and hold it until the lifetime cap.
//! - `--no-port` listens on nothing: it prints `PORT 0` and sleeps, for tests
//!   that need a process without a port.
//! - `--port <n>` binds that port instead of a free one, and `--v6` binds
//!   `[::1]` instead of `127.0.0.1`. Two copies, one of each, hold one port
//!   number through two different sockets.
//!
//! - `--share` (Unix only) forks after binding and prints `CHILD <pid>`. Both
//!   processes keep the listening socket open, as an nginx master and its
//!   workers do, so both hold the port.
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
    if flag("--takeover") {
        takeover();
        return;
    }
    if flag("--no-port") {
        println!("PORT 0");
        std::io::stdout().flush().expect("flush stdout");
        let deadline = Instant::now() + LIFETIME;
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(500));
        }
        return;
    }

    let port_arg = std::env::args()
        .skip_while(|a| a != "--port")
        .nth(1)
        .unwrap_or_else(|| "0".to_string());
    let host = if flag("--v6") { "[::1]" } else { "127.0.0.1" };
    let listener = TcpListener::bind(format!("{host}:{port_arg}")).expect("bind the listener");
    let port = listener.local_addr().expect("local addr").port();
    println!("PORT {port}");
    std::io::stdout().flush().expect("flush stdout");

    let deadline = Instant::now() + LIFETIME;
    if flag("--share") {
        share_socket();
    }
    let listener = if flag("--respawn") {
        match respawn_when_free(listener, port, deadline) {
            Some(listener) => listener,
            None => return,
        }
    } else {
        listener
    };

    // Keep the socket open for as long as we live
    let busy = flag("--busy");
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

/// Fork; the parent prints the child's pid and both carry on holding the
/// socket, each until the lifetime cap.
#[cfg(unix)]
fn share_socket() {
    // SAFETY: the process is single-threaded here, so fork is safe, and both
    // sides only go on to sleep.
    let pid = unsafe { libc::fork() };
    assert!(pid >= 0, "fork failed");
    if pid > 0 {
        println!("CHILD {pid}");
        std::io::stdout().flush().expect("flush stdout");
    }
}

#[cfg(not(unix))]
fn share_socket() {}

/// Fork. The parent prints the child's pid and gets its `listener` back. The
/// child closes its copy and waits for the port to be free, then binds it and
/// gets that listener; it gives up (`None`) at the `deadline`.
#[cfg(unix)]
fn respawn_when_free(listener: TcpListener, port: u16, deadline: Instant) -> Option<TcpListener> {
    // SAFETY: the process is single-threaded here, so fork is safe, and the
    // child only goes on to poll and sleep.
    let pid = unsafe { libc::fork() };
    assert!(pid >= 0, "fork failed");
    if pid > 0 {
        println!("CHILD {pid}");
        std::io::stdout().flush().expect("flush stdout");
        return Some(listener);
    }
    drop(listener);
    while Instant::now() < deadline {
        if let Ok(taken) = TcpListener::bind(("127.0.0.1", port)) {
            return Some(taken);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    None
}

/// Windows cannot fork: start a copy of this program that waits for the port
/// (`--takeover`). It is given no handle of ours (null standard streams, and a
/// listening socket is not inherited), so it holds nothing until the port is
/// free. The parent prints the copy's pid and gets its `listener` back.
#[cfg(not(unix))]
fn respawn_when_free(listener: TcpListener, port: u16, _deadline: Instant) -> Option<TcpListener> {
    // Never waited for on purpose: it must outlive this process, and Windows
    // keeps no zombie for a child nobody waits on
    #[allow(clippy::zombie_processes)]
    let child = std::process::Command::new(std::env::current_exe().expect("own path"))
        .args(["--takeover", &port.to_string()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("start the takeover copy");
    println!("CHILD {}", child.id());
    std::io::stdout().flush().expect("flush stdout");
    Some(listener)
}

/// `--takeover <port>`: bind the port as soon as it is free and hold it until
/// the lifetime cap
fn takeover() {
    let port: u16 = std::env::args()
        .skip_while(|a| a != "--takeover")
        .nth(1)
        .and_then(|p| p.parse().ok())
        .expect("--takeover <port>");
    let deadline = Instant::now() + LIFETIME;
    while Instant::now() < deadline {
        if let Ok(taken) = TcpListener::bind(("127.0.0.1", port)) {
            while Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(500));
            }
            drop(taken);
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
