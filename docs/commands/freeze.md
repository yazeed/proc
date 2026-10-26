---
layout: page
title: proc freeze
description: "Pause processes with SIGSTOP."
permalink: /commands/freeze
---

Send SIGSTOP to pause processes. They remain in memory but stop executing. Asks for confirmation unless `--yes`.

```bash
proc freeze node             # freeze all node processes
proc freeze :3000            # freeze process on port 3000
proc freeze :3000,:8080,node # freeze multiple targets
```

## Options

| Flag | Short | Description |
|------|-------|-------------|
| `--yes` | `-y` | Skip confirmation |
| `--dry-run` | | Show what would be frozen |
| `--in [<dir>]` | `-i` | Filter by directory |
| `--by <name>` | `-b` | Filter by process name |
| `--json` | `-j` | Output as JSON |
| `--verbose` | `-v` | Show extra details |

## Examples

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

## See also

[thaw](thaw) to resume frozen processes, [stop](stop) for graceful shutdown, [kill](kill) for immediate termination.
