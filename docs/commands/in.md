---
layout: page
title: proc in
description: "filter processes by working directory."
permalink: /commands/in
---

find processes running in a specific directory.

```bash
proc in .                  # processes in current directory
proc in ~/projects/myapp   # processes in a specific path
proc in . --by node        # node processes in cwd
```

The directory can be one that was deleted, such as a removed git worktree: processes still running there are listed. A path that doesn't exist and has nothing running in it is treated as a typo and exits 4.

## options

| flag | short | description |
|------|-------|-------------|
| `--by <name>` | `-b` | filter by process name |
| `--min-cpu <n>` | | minimum CPU % |
| `--min-mem <n>` | | minimum memory (MB) |
| `--status <s>` | | filter by status: running, sleeping, stopped, zombie |
| `--min-uptime <s>` | | minimum uptime in seconds |
| `--parent <pid>` | | only children of this parent PID |
| `--sort <key>` | `-s` | sort by: cpu, mem, pid, name (default: cpu) |
| `--limit <n>` | `-n` | limit results |
| `--json` | `-j` | output as JSON |
| `--verbose` | `-v` | show command line, cwd, parent PID, uptime |

## examples

```bash
# everything running in current project
proc in .

# only python processes in this directory
proc in . --by python

# high-CPU processes in a project
proc in ~/Sites/myapp --min-cpu 10

# JSON output for scripting
proc in . --json
```

## see also

[by](by) to start from a name, [list](list) for all processes.
