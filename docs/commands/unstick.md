---
layout: page
title: proc unstick
description: "attempt to recover stuck processes."
permalink: /commands/unstick
---

try to recover stuck processes before resorting to killing. alias: `u`.

sends SIGCONT then SIGINT recovery sequence. use `--force` to terminate if recovery fails.

```bash
proc unstick               # recover all stuck processes
proc unstick node          # recover stuck node processes
proc unstick --force       # terminate if recovery fails
```

## options

| flag | short | description |
|------|-------|-------------|
| `--timeout <secs>` | `-t` | seconds of high CPU before considered stuck (default: 300) |
| `--force` | `-f` | terminate if recovery fails |
| `--yes` | `-y` | skip confirmation |
| `--dry-run` | | show what would be done |
| `--in [<dir>]` | `-i` | filter by directory |
| `--by <name>` | `-b` | filter by process name |
| `--json` | `-j` | output as JSON |
| `--verbose` | `-v` | show extra details |

## examples

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

## see also

[stuck](stuck) for finding stuck processes, [stop](stop) for graceful shutdown.
