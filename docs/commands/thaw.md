---
layout: page
title: proc thaw
description: "Resume frozen processes with SIGCONT."
permalink: /commands/thaw
---

Send SIGCONT to resume processes paused by `proc freeze`. Asks for confirmation unless `--yes`. Not available on Windows: it exits 4.

```bash
proc thaw node             # resume all frozen node processes
proc thaw :3000            # resume frozen process on port 3000
proc thaw :3000,:8080,node # resume multiple targets
```

## Options

| Flag | Short | Description |
|------|-------|-------------|
| `--yes` | `-y` | Skip confirmation |
| `--dry-run` | | Show what would be resumed |
| `--in [<dir>]` | `-i` | Filter by directory |
| `--by <name>` | `-b` | Filter by process name |
| `--json` | `-j` | Output as JSON |
| `--verbose` | `-v` | Show extra details |

## Examples

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

## See also

[freeze](freeze) to pause processes, [stop](stop) for graceful shutdown.
