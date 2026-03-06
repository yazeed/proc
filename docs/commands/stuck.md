---
layout: page
title: proc stuck
description: "find stuck/hung processes."
permalink: /commands/stuck
---

find processes that appear stuck or hung. alias: `x`.

```bash
proc stuck                 # find all stuck processes
proc stuck --kill          # find and kill them
proc stuck --by node       # only stuck node processes
```

## options

| flag | short | description |
|------|-------|-------------|
| `--timeout <secs>` | `-t` | seconds to consider stuck (default: 300) |
| `--kill` | `-k` | kill found stuck processes |
| `--dry-run` | | show what would be killed |
| `--yes` | `-y` | skip confirmation when killing |
| `--in [<dir>]` | `-i` | filter by directory |
| `--by <name>` | `-b` | filter by process name |
| `--json` | `-j` | output as JSON |
| `--verbose` | `-v` | show extra details |

## examples

```bash
# find stuck processes (5-minute threshold)
proc stuck

# lower threshold to 60 seconds
proc stuck --timeout 60

# find and kill stuck processes
proc stuck --kill --yes

# preview what would be killed
proc stuck --kill --dry-run

# only stuck node processes in current project
proc stuck --by node --in .
```

## see also

[unstick](unstick) for recovery attempts before killing, [kill](kill) for direct termination.
