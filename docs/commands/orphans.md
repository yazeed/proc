---
layout: page
title: proc orphans
description: "find orphaned processes."
permalink: /commands/orphans
---

find processes whose parent has exited, leaving them orphaned (reparented to PID 1). alias: `o`.

```bash
proc orphans                 # list all orphaned processes
proc orphans --kill --yes    # kill all orphans without confirmation
proc orphans --by node       # find orphaned node processes
```

## options

| flag | short | description |
|------|-------|-------------|
| `--kill` | `-k` | kill orphaned processes |
| `--yes` | `-y` | skip confirmation (with --kill) |
| `--dry-run` | | show what would be killed (with --kill) |
| `--in [<dir>]` | `-i` | filter by directory |
| `--by <name>` | `-b` | filter by process name |
| `--json` | `-j` | output as JSON |
| `--verbose` | `-v` | show extra details |

## examples

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

## see also

[stuck](stuck) for hung processes, [unstick](unstick) to recover stuck processes.
