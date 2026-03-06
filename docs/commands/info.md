---
layout: page
title: proc info
description: "show detailed process information."
permalink: /commands/info
---

detailed information about specific processes. alias: `i`.

```bash
proc info :3000            # info for process on port 3000
proc info 1234             # info for PID
proc info node             # info for all node processes
proc info :3000,:8080      # multiple targets
```

## options

| flag | short | description |
|------|-------|-------------|
| `--in [<dir>]` | `-i` | filter by directory |
| `--by <name>` | `-b` | filter by process name |
| `--json` | `-j` | output as JSON |
| `--verbose` | `-v` | show extra details |

## output

shows for each process: name, PID, directory, executable path, user, parent PID, status, CPU %, memory, and uptime.

## examples

```bash
# what's on port 3000? (detailed)
proc info :3000

# info for multiple targets at once
proc info :3000,1234,node

# JSON for scripting
proc info node --json
```

## see also

[on](on) for quick port lookup, [list](list) for tabular overview.
