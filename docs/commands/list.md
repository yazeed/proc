---
layout: page
title: proc list
description: "list processes with filters and sorting."
permalink: /commands/list
---

list all processes, with optional filters. aliases: `l`, `ps`.

```bash
proc list                      # all processes
proc list node                 # filter by name
proc list --sort cpu --limit 10  # top 10 by CPU
```

## options

| flag | short | description |
|------|-------|-------------|
| `--in [<dir>]` | `-i` | filter by directory |
| `--path <path>` | `-p` | filter by executable path |
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
# top 10 processes by CPU
proc list --sort cpu --limit 10

# node and python processes
proc list node,python

# high-memory processes in current project
proc list --in . --min-mem 100

# running processes sorted by memory
proc list --status running --sort mem
```

## see also

[by](by) and [in](in) for targeted filtering, [watch](watch) for real-time monitoring.
