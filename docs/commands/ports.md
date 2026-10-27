---
layout: page
title: proc ports
description: "List all listening ports."
permalink: /commands/ports
---

Show all listening ports on the system. Alias: `p`.

```bash
proc ports                 # all listening ports
proc ports --by node       # ports used by processes named node
proc ports --exposed       # network-exposed only
```

## Options

| Flag | Short | Description |
|------|-------|-------------|
| `--by <name>` | `-b` | Filter by process name (the whole name, ignoring case) |
| `--cmd` | | Match `--by` as a substring of the process name or full command line (like `pkill -f`) |
| `--in [<dir>]` | `-i` | Filter by directory |
| `--exposed` | `-e` | Only network-exposed ports (0.0.0.0, ::) |
| `--local` | `-l` | Only localhost ports (127.0.0.1, ::1) |
| `--range <start-end>` | `-r` | Filter by port range (e.g., 3000-9000) |
| `--sort <key>` | `-s` | Sort by: port, pid, name (default: port) |
| `--limit <n>` | `-n` | Limit results |
| `--json` | `-j` | Output as JSON |
| `--verbose` | `-v` | Show executable path |

## Examples

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

## See also

[on](on) for looking up a specific port.
