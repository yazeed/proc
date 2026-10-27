---
layout: page
title: proc stuck
description: "Find stuck/hung processes."
permalink: /commands/stuck
---

Find processes that appear stuck or hung. Alias: `x`.

```bash
proc stuck                 # find all stuck processes
proc stuck --kill          # find and kill them
proc stuck --by node       # only stuck node processes
```

## Options

| Flag | Short | Description |
|------|-------|-------------|
| `--timeout <secs>` | `-t` | Only processes running longer than this, and over 50% CPU (default: 300) |
| `--kill` | `-k` | Kill found stuck processes |
| `--dry-run` | | Show what would be killed |
| `--yes` | `-y` | Skip confirmation when killing |
| `--in [<dir>]` | `-i` | Filter by directory |
| `--by <name>` | `-b` | Filter by process name |
| `--json` | `-j` | Output as JSON |
| `--verbose` | `-v` | Show extra details |
| `--wide` | `-w` | Print full directories and arguments instead of fitting the terminal |

## Examples

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

## See also

[unstick](unstick) for recovery attempts before killing, [kill](kill) for direct termination.
