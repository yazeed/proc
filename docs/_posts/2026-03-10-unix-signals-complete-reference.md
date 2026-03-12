---
title: "unix signals: the complete reference"
description: "Every Unix signal explained: what it does, its number, whether it can be caught, and when to use it. Plus signal handling in bash, C, Node, Python, and Go."
date: 2026-03-10
---

Signals are software interrupts. The kernel (or another process) sends a signal to a process to notify it that something happened. The process can catch the signal and handle it, ignore it, or let the default action occur.

Most developers know `kill -9` and `Ctrl+C`. There's more to it.

## signals every developer should know

### SIGTERM (15) — "please exit"

The default signal sent by `kill`. Asks the process to terminate cleanly. The process can catch this, run cleanup code (close files, flush buffers, release locks), and exit gracefully. This is what you should send first.

```bash
kill 12345        # sends SIGTERM
kill -15 12345    # same thing, explicit
```

### SIGKILL (9) — "exit now"

Uncatchable. The kernel terminates the process immediately. No cleanup handlers run, no files get flushed, no child processes get notified. Use this only when SIGTERM doesn't work.

```bash
kill -9 12345     # last resort
```

SIGKILL cannot be caught, blocked, or ignored. The kernel handles it directly — the target process never sees it.

### SIGINT (2) — "interrupt"

Sent when you press `Ctrl+C`. Like SIGTERM, it can be caught. Many programs use it to trigger a clean shutdown. The difference from SIGTERM is convention: SIGINT means "the user pressed Ctrl+C" while SIGTERM means "please exit."

### SIGHUP (1) — "hangup"

Originally meant "the terminal disconnected." Now has two common uses:

1. **Terminal close:** When you close a terminal window or SSH disconnects, the kernel sends SIGHUP to the session leader, which forwards it to child processes. This is why background processes die when you close your terminal (and why `nohup` exists).

2. **Config reload:** By convention, many daemons (nginx, Apache, sshd) reload their configuration on SIGHUP instead of exiting. This lets you apply config changes without downtime.

```bash
kill -HUP $(pgrep nginx)    # reload nginx config
```

### SIGSTOP (19) and SIGCONT (18) — "freeze and resume"

SIGSTOP freezes a process. Like SIGKILL, it **cannot be caught** — the process is unconditionally suspended. SIGCONT resumes it.

```bash
kill -STOP 12345    # freeze
kill -CONT 12345    # resume
```

`Ctrl+Z` in a terminal sends SIGTSTP (note: TST**P**, not STOP). Unlike SIGSTOP, SIGTSTP *can* be caught. The shell then sends SIGSTOP internally. `fg` and `bg` send SIGCONT.

### SIGPIPE (13) — "broken pipe"

Sent when a process writes to a pipe or socket whose read end has closed. The default action is to terminate the process — which is why piping to `head` doesn't cause infinite output:

```bash
cat /dev/urandom | head -c 100
```

`cat` gets SIGPIPE when `head` exits after reading 100 bytes. Without SIGPIPE, `cat` would keep writing to a dead pipe and error out.

**Gotcha:** SIGPIPE silently kills processes in scripts. If you pipe output to a command that exits early, the upstream process dies without any error message. In long-running services, this is a common source of mysterious crashes. Most servers ignore SIGPIPE and handle the write error directly.

### SIGSEGV (11) — "segmentation fault"

Sent when a process accesses memory it shouldn't (null pointer dereference, buffer overflow, etc.). The default action is to terminate and dump core. You'll see this as "Segmentation fault (core dumped)."

This is not a signal you send — the kernel sends it to the offending process.

### SIGCHLD (20) — "child exited"

Sent to a parent process when a child process exits or stops. This is how the kernel tells the parent to call `wait()` and collect the child's exit status. If the parent doesn't handle SIGCHLD, the child becomes a zombie.

### SIGUSR1 (30) and SIGUSR2 (31) — "user-defined"

No predefined meaning. Applications define their own behavior. Common uses:

- **Node.js:** SIGUSR1 starts the debugger
- **dd:** SIGUSR1 prints transfer progress
- **nginx:** SIGUSR2 triggers binary upgrade

```bash
kill -USR1 $(pgrep dd)    # print dd progress
```

## the uncatchable two

Only two signals cannot be caught, blocked, or ignored: **SIGKILL** and **SIGSTOP**. The kernel enforces this — the signal delivery mechanism bypasses the process entirely.

This means:
- A process in an infinite loop can always be killed (SIGKILL)
- A runaway process can always be frozen (SIGSTOP)
- No process can make itself unkillable (with one exception: uninterruptible sleep / D state, where signals are queued until I/O completes)

## signal handling in practice

### bash

```bash
trap 'echo "caught SIGTERM"; cleanup; exit' TERM
trap 'echo "caught SIGINT"; exit 1' INT
trap '' PIPE    # ignore SIGPIPE
```

### C

```c
#include <signal.h>

void handler(int sig) {
    // minimal work here — only async-signal-safe functions
    write(STDOUT_FILENO, "caught\n", 7);
    _exit(0);
}

signal(SIGTERM, handler);     // simple but has portability issues
sigaction(SIGTERM, &sa, NULL); // preferred — more control
```

### Node.js

```javascript
process.on('SIGTERM', () => {
  console.log('shutting down gracefully');
  server.close(() => process.exit(0));
});
// SIGKILL cannot be caught — this does nothing:
process.on('SIGKILL', () => {}); // never fires
```

### Python

```python
import signal, sys

def handler(signum, frame):
    print("shutting down")
    sys.exit(0)

signal.signal(signal.SIGTERM, handler)
```

### Go

```go
sigChan := make(chan os.Signal, 1)
signal.Notify(sigChan, syscall.SIGTERM, syscall.SIGINT)
go func() {
    sig := <-sigChan
    fmt.Printf("received %s, shutting down\n", sig)
    cleanup()
    os.Exit(0)
}()
```

## standard signals are not queued

If the same signal is sent twice before the handler runs, the process only sees it once. Standard signals (1-31) are represented as a bitmask — the bit is set on delivery and cleared when handled. Pending duplicates are lost.

Real-time signals (32-64) are queued and delivered in order, but most applications never use them.

## full reference table

| # | signal | default action | catchable? | common use |
|---|--------|---------------|------------|------------|
| 1 | SIGHUP | terminate | yes | terminal hangup / config reload |
| 2 | SIGINT | terminate | yes | Ctrl+C |
| 3 | SIGQUIT | terminate + core | yes | Ctrl+\\ (quit with core dump) |
| 4 | SIGILL | terminate + core | yes | illegal instruction |
| 5 | SIGTRAP | terminate + core | yes | debugger breakpoint |
| 6 | SIGABRT | terminate + core | yes | abort() called |
| 8 | SIGFPE | terminate + core | yes | floating point exception |
| 9 | SIGKILL | terminate | **no** | force kill |
| 10 | SIGBUS | terminate + core | yes | bus error (bad memory access) |
| 11 | SIGSEGV | terminate + core | yes | segmentation fault |
| 13 | SIGPIPE | terminate | yes | broken pipe |
| 14 | SIGALRM | terminate | yes | alarm() timer expired |
| 15 | SIGTERM | terminate | yes | graceful shutdown |
| 17 | SIGCHLD | ignore | yes | child process exited |
| 18 | SIGCONT | continue | yes | resume stopped process |
| 19 | SIGSTOP | stop | **no** | freeze process |
| 20 | SIGTSTP | stop | yes | Ctrl+Z |
| 21 | SIGTTIN | stop | yes | background read from terminal |
| 22 | SIGTTOU | stop | yes | background write to terminal |
| 23 | SIGURG | ignore | yes | urgent socket data |
| 26 | SIGVTALRM | terminate | yes | virtual timer expired |
| 27 | SIGPROF | terminate | yes | profiling timer expired |
| 28 | SIGWINCH | ignore | yes | terminal window resize |
| 29 | SIGIO | terminate | yes | I/O possible |
| 30 | SIGUSR1 | terminate | yes | user-defined |
| 31 | SIGUSR2 | terminate | yes | user-defined |

Note: signal numbers vary by platform (the above are for macOS/BSD). Linux uses different numbers for some signals (e.g., SIGCHLD is 17 on Linux, 20 on macOS). Use signal names, not numbers, in portable scripts.

## with proc

[proc](https://github.com/yazeed/proc)'s `stop` command sends SIGTERM first with a configurable timeout, then escalates to SIGKILL. `proc stuck` identifies processes consuming >50% CPU, and `proc unstick` sends recovery signals (SIGCONT, then SIGINT) before escalating.

```bash
proc stop node                 # SIGTERM, wait, then SIGKILL
proc stop node --timeout 10    # 10 second grace period
proc stuck                     # find high-CPU stuck processes
proc unstick                   # recover with escalating signals
```

## Install

```bash
brew install yazeed/proc/proc     # macOS
cargo install proc-cli            # Rust
npm install -g proc-cli           # npm/bun
```

See the [GitHub repo](https://github.com/yazeed/proc) for all installation options.
