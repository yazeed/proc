---
layout: page
title: proc unstick
description: "Attempt to recover stuck processes."
permalink: /commands/unstick
---

Try to recover stuck processes before resorting to killing. Alias: `u`.

Sends SIGCONT then SIGINT recovery sequence. Use `--force` to terminate if recovery fails.

```bash
proc unstick               # recover all stuck processes
proc unstick node          # recover stuck node processes
proc unstick --force       # terminate if recovery fails
```

## Options

| Flag | Short | Description |
|------|-------|-------------|
| `--timeout <secs>` | `-t` | Without a target: only stuck processes running longer than this (default: 300) |
| `--force` | `-f` | Terminate if recovery fails |
| `--yes` | `-y` | Skip confirmation |
| `--dry-run` | | Show what would be done |
| `--in [<dir>]` | `-i` | Filter by directory |
| `--by <name>` | `-b` | Filter by process name |
| `--json` | `-j` | Output as JSON |
| `--verbose` | `-v` | Show extra details |

## Examples

```bash
# try to recover all stuck processes
proc unstick

# recover specific process
proc unstick 1234

# force kill if recovery fails
proc unstick node --force

# preview actions without executing
proc unstick --dry-run
```

## See also

[stuck](stuck) for finding stuck processes, [stop](stop) for graceful shutdown.
