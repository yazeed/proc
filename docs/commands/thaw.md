---
layout: page
title: proc thaw
description: "resume frozen processes with SIGCONT."
permalink: /commands/thaw
---

send SIGCONT to resume processes paused by `proc freeze`. asks for confirmation unless `--yes`.

```bash
proc thaw node             # resume all frozen node processes
proc thaw :3000            # resume frozen process on port 3000
proc thaw :3000,:8080,node # resume multiple targets
```

## options

| flag | short | description |
|------|-------|-------------|
| `--yes` | `-y` | skip confirmation |
| `--dry-run` | | show what would be resumed |
| `--in [<dir>]` | `-i` | filter by directory |
| `--by <name>` | `-b` | filter by process name |
| `--json` | `-j` | output as JSON |
| `--verbose` | `-v` | show extra details |

## examples

```bash
# resume all frozen node processes
proc thaw node

# resume frozen process on a specific port
proc thaw :3000

# preview before resuming
proc thaw node --dry-run

# resume without confirmation
proc thaw :3000 --yes
```

## see also

[freeze](freeze) to pause processes, [stop](stop) for graceful shutdown.
