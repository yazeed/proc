---
layout: page
title: commands
description: "Complete reference for all proc commands."
permalink: /commands
---

## Discovery

| Command | Aliases | Description |
|---------|---------|-------------|
| [on](on) | `:` | Port/process lookup (bidirectional) |
| [why](why) | | Trace why a port is busy or show process ancestry |
| [for](for) | `f` | Find processes by file path |
| [by](by) | `b` | Filter processes by name |
| [in](in) | | Filter processes by working directory |
| [list](list) | `l`, `ps` | List processes |
| [info](info) | `i`, `inspect` | Show detailed process information |
| [ports](ports) | `p` | List all listening ports |
| [tree](tree) | `t` | Show process tree |
| [watch](watch) | `w`, `top` | Real-time process monitoring |
| [stuck](stuck) | `x` | Find stuck/hung processes |
| [orphans](orphans) | `o` | Find orphaned processes |

## Lifecycle

| Command | Aliases | Description |
|---------|---------|-------------|
| [kill](kill) | `k` | Force kill processes (SIGKILL) |
| [stop](stop) | `s` | Graceful stop with --signal support (SIGTERM, then SIGKILL) |
| [freeze](freeze) | | Pause processes (SIGSTOP) |
| [thaw](thaw) | | Resume frozen processes (SIGCONT) |
| [free](free) | | Free ports by killing the occupying process |
| [wait](wait) | | Block until process(es) exit |
| [unstick](unstick) | `u` | Attempt to recover stuck processes |

## Common options

Most commands support these flags (check individual command pages for details):

| Flag | Short | Description |
|------|-------|-------------|
| `--json` | `-j` | Output as JSON |
| `--verbose` | `-v` | Show extra details |
| `--in <dir>` | `-i` | Filter by directory |
| `--by <name>` | `-b` | Filter by process name |
| `--help` | `-h` | Show help |

## Target syntax

Targets work the same way everywhere:

```bash
proc on :3000          # port
proc on 1234           # PID
proc on node           # name
proc kill :3000,:8080  # multiple (comma-separated)
```

## Scripting

With `--json`, every command prints one JSON document with `action` (the command name) and `success`. `proc watch --json` streams one per refresh. Errors, including invalid flags, are JSON too. A command that changes processes run with `--json` but without `--yes` exits 4 and adds `"needs_confirmation": true` and `"processes"` (the ones it would have changed).

| Exit code | Meaning |
|-----------|---------|
| `0` | Success |
| `1` | Error, or not fully successful: a process failed, a port stayed busy, `wait` timed out |
| `2` | Process or port not found |
| `3` | Permission denied |
| `4` | Invalid input (unknown flag, bad argument, invalid signal), an action this OS doesn't support (`freeze`, `thaw` and `stop --signal` on Windows), or a command that changes processes run without `--yes` where nobody can confirm (`--json`, or no terminal): nothing was changed |

`"success": false` always comes with a non-zero exit code.
