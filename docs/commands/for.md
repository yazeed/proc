---
layout: page
title: proc for
description: "Find processes by file path."
permalink: /commands/for
---

Find processes running a specific file. Alias: `f`.

```bash
proc for ./script.py       # what's running this file?
proc for /usr/bin/node     # processes using this executable
proc for ~/bin/myapp       # tilde expansion
```

One process gets the same detail view as `proc on :port` (directory, path, listening ports, resources, uptime). Several get the standard process table, followed by the ports they listen on.

## Options

| Flag | Short | Description |
|------|-------|-------------|
| `--in [<dir>]` | `-i` | Filter by working directory |
| `--by <name>` | `-b` | Filter by process name |
| `--min-cpu <n>` | | Minimum CPU % |
| `--min-mem <n>` | | Minimum memory (MB) |
| `--status <s>` | | Filter by status: running, sleeping, stopped, zombie |
| `--min-uptime <s>` | | Minimum uptime in seconds |
| `--sort <key>` | `-s` | Sort by: cpu, mem, pid, name (default: cpu) |
| `--limit <n>` | `-n` | Limit results |
| `--json` | `-j` | Output as JSON |
| `--verbose` | `-v` | Show extra details |
| `--wide` | `-w` | Print full directories and arguments instead of fitting the terminal |

## Examples

```bash
# what's running this script?
proc for ./app.js

# find processes with a data file open
proc for ./data.json

# high-CPU processes running a specific binary
proc for /usr/bin/python3 --min-cpu 10
```

## See also

[by](by) to filter by name, [in](in) to filter by directory.
