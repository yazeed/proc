---
layout: page
title: proc info
description: "Show detailed process information."
permalink: /commands/info
---

Detailed information about specific processes. Aliases: `i`, `inspect`.

```bash
proc info :3000            # info for process on port 3000
proc info 1234             # info for PID
proc info node             # info for all node processes
proc info :3000 :8080      # multiple targets
```

Several targets are separate arguments. Comma-separated lists (`:3000,:8080`) still work but are deprecated.

## Options

| Flag | Short | Description |
|------|-------|-------------|
| `--in [<dir>]` | `-i` | Filter by directory. Takes the next word as its directory unless it starts with `-` (bare `--in` is the current directory, `--in=DIR` is explicit) |
| `--by <name>` | `-b` | Filter by process name |
| `--json` | `-j` | Output as JSON |
| `--verbose` | `-v` | Show extra details |

## Output

Shows for each process: name, PID, directory, executable path, user, parent PID, status, CPU %, memory, and uptime.

## Examples

```bash
# what's on port 3000? (detailed)
proc info :3000

# info for multiple targets at once
proc info :3000 1234 node

# JSON for scripting
proc info node --json
```

## See also

[on](on) for quick port lookup, [list](list) for tabular overview.
