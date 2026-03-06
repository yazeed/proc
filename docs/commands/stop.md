---
layout: page
title: proc stop
description: "gracefully stop processes."
permalink: /commands/stop
---

send SIGTERM, wait for exit, then SIGKILL after timeout. alias: `s`.

```bash
proc stop :3000            # gracefully stop port 3000
proc stop node             # stop all node processes
proc stop node --timeout 5 # 5s before force kill
```

## options

| flag | short | description |
|------|-------|-------------|
| `--yes` | `-y` | skip confirmation |
| `--dry-run` | | show what would be stopped |
| `--timeout <secs>` | `-t` | seconds before force kill (default: 10) |
| `--in [<dir>]` | `-i` | filter by directory |
| `--by <name>` | `-b` | filter by process name |
| `--json` | `-j` | output as JSON |
| `--verbose` | `-v` | show extra details |

## examples

```bash
# graceful stop with 5-second timeout
proc stop node --timeout 5

# preview what would be stopped
proc stop node --dry-run

# stop processes in current project only
proc stop node --in .

# stop without confirmation
proc stop :3000 --yes
```

## see also

[kill](kill) for immediate SIGKILL, [unstick](unstick) for stuck processes.
