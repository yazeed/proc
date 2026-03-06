---
layout: page
title: proc ports
description: "list all listening ports."
permalink: /commands/ports
---

show all listening ports on the system. alias: `p`.

```bash
proc ports                 # all listening ports
proc ports --by node       # ports used by node
proc ports --exposed       # network-exposed only
```

## options

| flag | short | description |
|------|-------|-------------|
| `--by <name>` | `-b` | filter by process name |
| `--in [<dir>]` | `-i` | filter by directory |
| `--exposed` | `-e` | only network-exposed ports (0.0.0.0, ::) |
| `--local` | `-l` | only localhost ports (127.0.0.1, ::1) |
| `--range <start-end>` | `-r` | filter by port range (e.g., 3000-9000) |
| `--sort <key>` | `-s` | sort by: port, pid, name (default: port) |
| `--limit <n>` | `-n` | limit results |
| `--json` | `-j` | output as JSON |
| `--verbose` | `-v` | show executable path |

## examples

```bash
# all ports, sorted by process name
proc ports --sort name

# ports in the 8000-9000 range
proc ports --range 8000-9000

# only externally accessible ports
proc ports --exposed

# ports from processes in current project
proc ports --in .
```

## see also

[on](on) for looking up a specific port.
