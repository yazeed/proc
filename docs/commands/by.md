---
layout: page
title: proc by
description: "Filter processes by name."
permalink: /commands/by
---

Find processes matching a name pattern. Alias: `b`.

```bash
proc by node               # all node processes
proc by python --in .      # python in current directory
proc by node --min-cpu 5   # node using >5% CPU
```

`by` takes one pattern, matched as a case-insensitive substring of the name or the full command line. For several patterns, use `proc list node python`. The table includes uptime on terminals at least 100 columns wide; `--verbose` and `--json` always include it.

## Options

| Flag | Short | Description |
|------|-------|-------------|
| `--in [<dir>]` | `-i` | Filter by directory. Takes the next word as its directory unless it starts with `-` (bare `--in` is the current directory, `--in=DIR` is explicit) |
| `--min-cpu <n>` | | Minimum CPU % |
| `--min-mem <n>` | | Minimum memory (MB) |
| `--status <s>` | | Filter by status: running, sleeping, stopped, zombie |
| `--min-uptime <s>` | | Minimum uptime in seconds |
| `--parent <pid>` | | Only children of this parent PID |
| `--sort <key>` | `-s` | Sort by: cpu, mem, pid, name (default: cpu) |
| `--limit <n>` | `-n` | Limit results |
| `--json` | `-j` | Output as JSON |
| `--verbose` | `-v` | Show command line, cwd, parent PID, uptime |
| `--wide` | `-w` | Print full directories and arguments instead of fitting the terminal |

## Examples

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

## See also

[in](in) to start from a directory, [list](list) for all processes.
