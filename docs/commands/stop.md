---
layout: page
title: proc stop
description: "Gracefully stop processes."
permalink: /commands/stop
---

Send SIGTERM, wait for exit, then SIGKILL after timeout. Alias: `s`.

```bash
proc stop :3000            # gracefully stop port 3000
proc stop node             # stop all node processes
proc stop node --timeout 5 # 5s before force kill
```

## Options

| Flag | Short | Description |
|------|-------|-------------|
| `--yes` | `-y` | Skip confirmation |
| `--dry-run` | | Show what would be stopped |
| `--timeout <secs>` | `-t` | Seconds before force kill (default: 10) |
| `--signal <name>` | `-S` | Send a specific signal instead of SIGTERM (macOS and Linux; exits 4 on Windows) |
| `--in [<dir>]` | `-i` | Filter by directory |
| `--by <name>` | `-b` | Filter by process name |
| `--json` | `-j` | Output as JSON |
| `--verbose` | `-v` | Show extra details |

## Examples

```bash
# graceful stop with 5-second timeout
proc stop node --timeout 5

# preview what would be stopped
proc stop node --dry-run

# stop processes in current project only
proc stop node --in .

# stop without confirmation
proc stop :3000 --yes

# send SIGHUP to reload nginx config
proc stop nginx --signal HUP

# send SIGUSR1 to a worker process
proc stop worker --signal USR1
```

## See also

[kill](kill) for immediate SIGKILL, [unstick](unstick) for stuck processes, [freeze](freeze)/[thaw](thaw) to pause and resume.
