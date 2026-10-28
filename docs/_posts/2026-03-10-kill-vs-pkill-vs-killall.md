---
title: "kill vs pkill vs killall: when to use each"
description: "The differences between kill, pkill, and killall. Signal delivery, regex matching, portability gotchas, and when each is the right choice."
date: 2026-03-10
---

Three commands, three different approaches to stopping processes. They look similar but behave differently in ways that matter.

## kill: signal by PID

`kill` sends a signal to a specific process by PID. That's it. Despite the name, it doesn't always kill — it's really a signal delivery mechanism.

```bash
kill 12345          # send SIGTERM (default)
kill -9 12345       # send SIGKILL
kill -HUP 12345     # send SIGHUP (reload config)
kill -STOP 12345    # freeze the process
kill -CONT 12345    # resume it
```

The common signals (numbers differ by OS, so use the names in scripts):

| signal | number | catchable? | use case |
|--------|--------|------------|----------|
| SIGTERM | 15 | yes | ask process to exit cleanly |
| SIGKILL | 9 | no | force kill, no cleanup |
| SIGINT | 2 | yes | same as Ctrl+C |
| SIGHUP | 1 | yes | reload config / terminal hangup |
| SIGSTOP | 17 (macOS) / 19 (Linux) | no | freeze process |
| SIGCONT | 19 (macOS) / 18 (Linux) | yes (it resumes the process anyway) | resume frozen process |

`kill` is the safest option because you're explicit about which PID gets the signal. No pattern matching, no ambiguity. The downside: you need the PID first.

```bash
# typical workflow
ps aux | grep node
# read the PID from the output
kill 12345
```

You can signal multiple PIDs:

```bash
kill 12345 12346 12347
kill -9 $(pgrep -f "webpack-dev-server")
```

## pkill: signal by pattern

`pkill` matches process names (or command lines) against a regex pattern and signals every match.

```bash
pkill node          # SIGTERM to all processes named "node"
pkill -9 webpack    # SIGKILL to all processes matching "webpack"
pkill -f "npm start" # match against full command line
```

Useful flags:

| flag | description |
|------|-------------|
| `-f` | match against full command line, not just process name |
| `-x` | exact match only (no partial matching) |
| `-u user` | only processes owned by this user |
| `-t tty` | only processes on this terminal |
| `-P pid` | only children of this parent PID |
| `-n` | only the newest matching process |
| `-o` | only the oldest matching process |

### the regex gotcha

`pkill` uses regex matching by default, and it matches **substrings**:

```bash
pkill node
```

This kills processes named `node`, but also `nodemon`, `nodecellar`, and anything else containing "node." If you want an exact match:

```bash
pkill -x node
```

The `-f` flag is powerful but dangerous. `pkill -f "server"` matches any process whose command line contains "server" — which could be a lot of things.

### pgrep: the read-only sibling

`pgrep` uses the same matching logic as `pkill` but only prints PIDs. Use it to preview what `pkill` would hit:

```bash
pgrep -l node        # list PIDs and names
pgrep -lf "npm start" # list PIDs and full commands
```

Always run `pgrep` before `pkill` if you're unsure what will match.

## killall: signal by exact name

`killall` matches by process name — but unlike `pkill`, it matches the **exact process name** (on Linux and macOS), not a regex substring.

```bash
killall node         # all processes exactly named "node"
killall -9 firefox   # force kill firefox
```

Useful flags:

| flag | description |
|------|-------------|
| `-w` | wait for processes to die before returning (Linux only) |
| `-I` | case-insensitive matching (Linux); on macOS, ask for confirmation before each signal |
| `-v` | report whether the signal was sent successfully |
| `-r` | use regex matching (Linux, makes it behave more like pkill; macOS uses `-m`) |

### the macOS warning

On Solaris and some System V systems, `killall` without arguments kills **all processes**. This is not a theoretical risk — it will log you out or worse. macOS and Linux both require a process name, but the reputation sticks.

More importantly, the flags differ between Linux and macOS, even though matching is exact on both by default:

```bash
killall node    # Linux: only "node", not "nodemon"
killall node    # macOS: also only "node"; -m switches to a regex
```

Linux has `-r`, `-I` (case-insensitive) and `-w`; macOS has `-m`, `-I` (confirm each signal) and no `-w`. If you write scripts that run on both platforms, `pkill -x` is more predictable.

### the -w flag (Linux)

One genuinely useful `killall` feature on Linux: the `-w` (wait) flag blocks until all matched processes have exited. Useful in scripts where you need to know the process is truly gone before continuing. macOS `killall` has no `-w`; use `proc wait node` there.

```bash
killall -w node && echo "all node processes stopped"
```

Neither `kill` nor `pkill` has this built in.

## when to use each

**Use `kill`** when you know the exact PID and want maximum control. Best for interactive use where you've already identified the process.

**Use `pkill`** when you want to match by name, command line, user, or parent. Best for scripts and automation. Always preview with `pgrep` first.

**Use `killall`** when you want exact name matching and, on Linux, the `-w` (wait) flag. Be aware of macOS/Linux differences. Avoid in cross-platform scripts.

## with proc

[proc](https://github.com/yazeed/proc-cli) handles name matching, port targeting, and multi-target resolution with deduplication and confirmation prompts:

```bash
proc kill node            # processes named exactly node (add --cmd to match node anywhere in the command line), with confirmation
proc stop :3000           # graceful stop by port
proc kill :3000 webpack   # multiple targets, deduplicated
proc kill node --dry-run  # preview what would be killed
```

Explicit intent, no regex surprises, confirmation before anything dies.

## Install

```bash
brew install yazeed/proc/proc     # macOS
cargo install proc-cli            # Rust
npm install -g proc-cli           # npm/bun
```

See the [GitHub repo](https://github.com/yazeed/proc-cli) for all installation options.
