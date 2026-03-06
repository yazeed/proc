---
layout: page
title: proc for
description: "find processes by file path."
permalink: /commands/for
---

find processes running a specific file. alias: `f`.

```bash
proc for ./script.py       # what's running this file?
proc for /usr/bin/node     # processes using this executable
proc for ~/bin/myapp       # tilde expansion
```

## options

| flag | short | description |
|------|-------|-------------|
| `--in [<dir>]` | `-i` | filter by working directory |
| `--by <name>` | `-b` | filter by process name |
| `--min-cpu <n>` | | minimum CPU % |
| `--min-mem <n>` | | minimum memory (MB) |
| `--status <s>` | | filter by status: running, sleeping, stopped, zombie |
| `--min-uptime <s>` | | minimum uptime in seconds |
| `--sort <key>` | `-s` | sort by: cpu, mem, pid, name (default: cpu) |
| `--limit <n>` | `-n` | limit results |
| `--json` | `-j` | output as JSON |
| `--verbose` | `-v` | show extra details |

## examples

```bash
# what's running this script?
proc for ./app.js

# find processes with a data file open
proc for ./data.json

# high-CPU processes running a specific binary
proc for /usr/bin/python3 --min-cpu 10
```

## see also

[by](by) to filter by name, [in](in) to filter by directory.
