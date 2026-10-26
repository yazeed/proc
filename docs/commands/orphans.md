---
layout: page
title: proc orphans
description: "Find orphaned processes."
permalink: /commands/orphans
---

Find processes whose parent has exited, leaving them orphaned (reparented to PID 1). Alias: `o`.

```bash
proc orphans                 # list all orphaned processes
proc orphans --kill --yes    # kill all orphans without confirmation
proc orphans --by node       # find orphaned node processes
```

## Options

| Flag | Short | Description |
|------|-------|-------------|
| `--kill` | `-k` | Kill orphaned processes |
| `--yes` | `-y` | Skip confirmation (with --kill) |
| `--dry-run` | | Show what would be killed (with --kill) |
| `--in [<dir>]` | `-i` | Filter by directory |
| `--by <name>` | `-b` | Filter by process name |
| `--json` | `-j` | Output as JSON |
| `--verbose` | `-v` | Show extra details |

## Examples

```bash
# list all orphaned processes
proc orphans

# find orphans in the current project
proc orphans --in .

# find orphaned node processes
proc orphans --by node

# preview which orphans would be killed
proc orphans --kill --dry-run

# kill all orphans without confirmation
proc orphans --kill --yes
```

## See also

[stuck](stuck) for hung processes, [unstick](unstick) to recover stuck processes.
