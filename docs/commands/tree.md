---
layout: page
title: proc tree
description: "Show process tree."
permalink: /commands/tree
---

Display process hierarchy. Alias: `t`.

```bash
proc tree                  # full process tree
proc tree node             # tree rooted at node
proc tree :3000            # tree from port 3000's process
proc tree 1234 --ancestors # path up to root
```

Each line shows the process's status, name, PID, CPU, memory and uptime (`up 3m 12s`); `--json` includes `uptime_secs`.

## Options

| Flag | Short | Description |
|------|-------|-------------|
| `--ancestors` | `-a` | Show path up to root instead of descendants |
| `--depth <n>` | `-d` | Max depth to display (default: 10) |
| `--compact` | `-C` | Show PIDs only |
| `--min-cpu <n>` | | Minimum CPU % |
| `--min-mem <n>` | | Minimum memory (MB) |
| `--status <s>` | | Filter by status |
| `--min-uptime <s>` | | Minimum uptime in seconds |
| `--in [<dir>]` | `-i` | Filter by directory. Takes the next word as its directory unless it starts with `-` (bare `--in` is the current directory, `--in=DIR` is explicit) |
| `--by <name>` | `-b` | Filter by process name |
| `--json` | `-j` | Output as JSON |
| `--verbose` | `-v` | Show extra details |

## Examples

```bash
# show node's process tree
proc tree node

# ancestry of process on port 3000
proc tree :3000 --ancestors

# shallow tree (2 levels deep)
proc tree node --depth 2

# compact view (PIDs only)
proc tree node --compact
```

## See also

[list](list) for flat process listing, [info](info) for detailed single-process view.
