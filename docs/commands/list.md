---
layout: page
title: proc list
description: "List processes with filters and sorting."
permalink: /commands/list
---

List all processes, with optional filters. Aliases: `l`, `ps`.

```bash
proc list                      # all processes
proc list node                 # filter by name
proc list node python          # several names
proc list --sort cpu --limit 10  # top 10 by CPU
```

Several names are separate arguments. Comma-separated lists (`node,python`) still work but are deprecated.

The table shows PID, directory, name, arguments, CPU, memory and status, plus uptime on terminals at least 100 columns wide. `--verbose` and `--json` always include uptime.

One process is always one line. Directories and arguments are shortened to fit the terminal, with long paths keeping their root and their tail (`/Users/…/maliqe-website/.next/standalone`). Use `--wide` to print them in full.

## Options

| Flag | Short | Description |
|------|-------|-------------|
| `--in [<dir>]` | `-i` | Filter by directory. Takes the next word as its directory unless it starts with `-` (bare `--in` is the current directory, `--in=DIR` is explicit) |
| `--path <path>` | `-p` | Filter by executable path |
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
# top 10 processes by CPU
proc list --sort cpu --limit 10

# node and python processes
proc list node python

# high-memory processes in current project
proc list --in . --min-mem 100

# running processes sorted by memory
proc list --status running --sort mem
```

## See also

[by](by) and [in](in) for targeted filtering, [watch](watch) for real-time monitoring.
