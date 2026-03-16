---
layout: page
title: proc free
description: "free ports by killing the occupying process."
permalink: /commands/free
---

kill the process on a port and verify the port becomes available. port-only targets. asks for confirmation unless `--yes`.

```bash
proc free :3000            # free port 3000
proc free :3000,:8080      # free multiple ports
proc free :3000 --wait 30  # wait up to 30s for port to free
```

## options

| flag | short | description |
|------|-------|-------------|
| `--yes` | `-y` | skip confirmation |
| `--dry-run` | | show what would be freed |
| `--wait <secs>` | | seconds to wait for port availability (default: 10) |
| `--json` | `-j` | output as JSON |
| `--verbose` | `-v` | show extra details |

## examples

```bash
# free a single port
proc free :3000

# free multiple ports without confirmation
proc free :3000,:8080 --yes

# wait longer for port to become available
proc free :3000 --wait 30

# preview before freeing
proc free :3000 --dry-run
```

## see also

[kill](kill) for force kill without port verification, [stop](stop) for graceful shutdown, [on](on) to check what's on a port.
