---
layout: page
title: proc wait
description: "Wait for processes to exit."
permalink: /commands/wait
---

Block until target process(es) exit. Non-interactive — works in scripts and pipelines.

```bash
proc wait node                 # wait until all node processes exit
proc wait :3000                # wait until port 3000's process exits
proc wait 1234                 # wait for specific PID to exit
proc wait node,python          # wait for all to exit
```

## Options

| Flag | Short | Description |
|------|-------|-------------|
| `--interval <secs>` | `-n` | Poll interval in seconds (default: 5) |
| `--timeout <secs>` | `-t` | Max wait time, 0 = no timeout (default: 0). Exits 1 if processes are still running |
| `--quiet` | `-q` | Suppress periodic status, only print exits and final result |
| `--in [<dir>]` | `-i` | Filter by directory |
| `--by <name>` | `-b` | Filter by process name |
| `--json` | `-j` | Output as JSON |
| `--verbose` | `-v` | Show extra details |

## Examples

```bash
# wait for node processes to finish
proc wait node

# wait with 60-second timeout
proc wait node --timeout 60

# check every 10 seconds, quiet mode (for scripts)
proc wait node -n 10 -q

# wait for a port to become free
proc wait :3000

# wait for multiple targets
proc wait node,python --timeout 3600

# wait for processes in current project
proc wait node --in .

# json output for scripting (exit 1 and "timed_out": true on timeout)
proc wait node --json --timeout 120 || echo "still running"
```

## See also

[watch](watch) for real-time interactive monitoring, [free](free) to kill and verify port freed, [by](by) to find processes by name.
