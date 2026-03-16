---
layout: page
title: proc freeze
description: "pause processes with SIGSTOP."
permalink: /commands/freeze
---

send SIGSTOP to pause processes. they remain in memory but stop executing. asks for confirmation unless `--yes`.

```bash
proc freeze node             # freeze all node processes
proc freeze :3000            # freeze process on port 3000
proc freeze :3000,:8080,node # freeze multiple targets
```

## options

| flag | short | description |
|------|-------|-------------|
| `--yes` | `-y` | skip confirmation |
| `--dry-run` | | show what would be frozen |
| `--in [<dir>]` | `-i` | filter by directory |
| `--by <name>` | `-b` | filter by process name |
| `--json` | `-j` | output as JSON |
| `--verbose` | `-v` | show extra details |

## examples

```bash
# freeze all node processes
proc freeze node

# freeze process on a specific port
proc freeze :3000

# preview before freezing
proc freeze node --dry-run

# freeze processes in current project only
proc freeze node --in .

# freeze without confirmation
proc freeze :3000 --yes
```

## see also

[thaw](thaw) to resume frozen processes, [stop](stop) for graceful shutdown, [kill](kill) for immediate termination.
