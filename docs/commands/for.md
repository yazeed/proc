---
layout: page
title: proc for
description: "Find processes by file path."
permalink: /commands/for
---

Find processes running a specific file. Alias: `f`.

```bash
proc for ./server          # what's running this program (or holding it open)?
proc for /usr/bin/node     # processes using this executable
proc for ~/bin/myapp       # tilde expansion
```

One process gets the same detail view as `proc on :port` (directory, path, listening ports, resources, uptime). Several get the standard process table, followed by the ports they listen on.

## Options

| Flag | Short | Description |
|------|-------|-------------|
| `--in [<dir>]` | `-i` | Filter by working directory. Takes the next word as its directory unless it starts with `-` (bare `--in` is the current directory, `--in=DIR` is explicit) |
| `--by <name>` | `-b` | Filter by process name (the whole name, ignoring case) |
| `--cmd` | | Match `--by` as a substring of the process name or full command line (like `pkill -f`) |
| `--min-cpu <n>` | | Minimum CPU % |
| `--min-mem <n>` | | Minimum memory (MB) |
| `--status <s>` | | Filter by status: running, sleeping, stopped, zombie |
| `--min-uptime <s>` | | Minimum uptime in seconds |
| `--sort <key>` | `-s` | Sort by: cpu, mem (biggest first); pid, name (ascending). Ties come by PID (default: cpu) |
| `--limit <n>` | `-n` | Limit results |
| `--json` | `-j` | Output as JSON |
| `--verbose` | `-v` | Show extra details |
| `--wide` | `-w` | Print full directories and arguments instead of fitting the terminal |

## Examples

```bash
# what's running this program?
proc for ./server

# find processes with a data file open
proc for ./data.json

# high-CPU processes running a specific binary
proc for /usr/bin/python3 --min-cpu 10
```

## See also

[by](by) to filter by name, [in](in) to filter by directory.
