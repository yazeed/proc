---
layout: page
title: proc kill
description: "Force kill processes."
permalink: /commands/kill
---

Send SIGKILL to processes. Asks for confirmation unless `--yes`. Alias: `k`.

```bash
proc kill :3000            # kill process on port 3000
proc kill node             # kill all node processes
proc kill :3000 :8080 node # kill multiple targets
```

Several targets are separate arguments. Comma-separated lists (`:3000,:8080`) still work but are deprecated.

## Options

| Flag | Short | Description |
|------|-------|-------------|
| `--yes` | `-y` | Skip confirmation |
| `--dry-run` | | Show what would be killed |
| `--graceful` | `-g` | Send SIGTERM instead of SIGKILL. On Windows it asks the program to close its windows, so a console program refuses (exit 1); use [`proc stop`](stop) for a stop that escalates |
| `--in [<dir>]` | `-i` | Filter by directory. Takes the next word as its directory unless it starts with `-` (bare `--in` is the current directory, `--in=DIR` is explicit) |
| `--by <name>` | `-b` | Filter by process name |
| `--json` | `-j` | Output as JSON |
| `--verbose` | `-v` | Show extra details |

## Examples

```bash
# preview before killing
proc kill node --dry-run

# kill without confirmation
proc kill :3000 --yes

# graceful shutdown (SIGTERM)
proc kill node --graceful

# only kill node in current project
proc kill node --in .

# kill multiple targets at once
proc kill :3000 :8080 node

# kill two PIDs and whatever is on port 8080
proc kill 1234 5678 :8080
```

## See also

[stop](stop) for graceful shutdown with timeout, [unstick](unstick) for stuck processes.
