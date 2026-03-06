---
layout: page
title: proc tree
description: "show process tree."
permalink: /commands/tree
---

display process hierarchy. alias: `t`.

```bash
proc tree                  # full process tree
proc tree node             # tree rooted at node
proc tree :3000            # tree from port 3000's process
proc tree 1234 --ancestors # path up to root
```

## options

| flag | short | description |
|------|-------|-------------|
| `--ancestors` | `-a` | show path up to root instead of descendants |
| `--depth <n>` | `-d` | max depth to display (default: 10) |
| `--compact` | `-C` | show PIDs only |
| `--min-cpu <n>` | | minimum CPU % |
| `--min-mem <n>` | | minimum memory (MB) |
| `--status <s>` | | filter by status |
| `--min-uptime <s>` | | minimum uptime in seconds |
| `--in [<dir>]` | `-i` | filter by directory |
| `--by <name>` | `-b` | filter by process name |
| `--json` | `-j` | output as JSON |
| `--verbose` | `-v` | show extra details |

## examples

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

## see also

[list](list) for flat process listing, [info](info) for detailed single-process view.
