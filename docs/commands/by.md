---
layout: page
title: proc by
description: "filter processes by name."
permalink: /commands/by
---

find processes matching a name pattern. alias: `b`.

```bash
proc by node               # all node processes
proc by python --in .      # python in current directory
proc by node --min-cpu 5   # node using >5% CPU
```

## options

| flag | short | description |
|------|-------|-------------|
| `--in [<dir>]` | `-i` | filter by directory |
| `--min-cpu <n>` | | minimum CPU % |
| `--min-mem <n>` | | minimum memory (MB) |
| `--status <s>` | | filter by status: running, sleeping, stopped, zombie |
| `--min-uptime <s>` | | minimum uptime in seconds |
| `--parent <pid>` | | only children of this parent PID |
| `--sort <key>` | `-s` | sort by: cpu, mem, pid, name (default: cpu) |
| `--limit <n>` | `-n` | limit results |
| `--json` | `-j` | output as JSON |
| `--verbose` | `-v` | show command line, cwd, parent PID |

## examples

```bash
# node processes in current project
proc by node --in .

# high-memory python processes
proc by python --min-mem 500

# sleeping processes named worker
proc by worker --status sleeping

# top 5 node processes by memory
proc by node --sort mem --limit 5
```

## see also

[in](in) to start from a directory, [list](list) for all processes.
