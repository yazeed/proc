//! A process that owns a listening port, for tests to aim proc at.
//!
//! Binds `127.0.0.1:0`, prints `PORT <n>` on stdout and flushes, then sleeps
//! until it is killed. `--ignore-term` ignores SIGTERM on Unix (a no-op on
//! Windows), for tests that need a process a graceful stop cannot move.

use std::io::Write;
use std::net::TcpListener;
use std::time::Duration;

fn main() {
    if std::env::args().any(|a| a == "--ignore-term") {
        ignore_term();
    }

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1:0");
    let port = listener.local_addr().expect("local addr").port();
    println!("PORT {port}");
    std::io::stdout().flush().expect("flush stdout");

    // Keep the socket open for as long as we live
    loop {
        std::thread::sleep(Duration::from_secs(60));
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
