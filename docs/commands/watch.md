---
layout: page
title: proc watch
description: "real-time process monitoring."
permalink: /commands/watch
---

live-updating process table. aliases: `w`, `top`.

```bash
proc watch                 # watch all processes
proc top                   # same thing
proc watch node            # watch node processes
proc watch :3000           # watch process on port
```

press `q`, `Esc`, or `Ctrl+C` to exit. terminal is always restored cleanly.

## options

| flag | short | description |
|------|-------|-------------|
| `--interval <secs>` | `-n` | refresh interval (default: 2) |
| `--sort <key>` | `-s` | sort by: cpu, mem, pid, name (default: cpu) |
| `--limit <n>` | `-l` | limit results shown |
| `--in [<dir>]` | `-i` | filter by directory |
| `--by <name>` | `-b` | filter by process name |
| `--min-cpu <n>` | | minimum CPU % |
| `--min-mem <n>` | | minimum memory (MB) |
| `--json` | `-j` | NDJSON output (one object per refresh) |
| `--verbose` | `-v` | show extra details |

## modes

**terminal (default)**: alternate screen with auto-refresh. adapts to terminal size.

**json** (`--json`): newline-delimited JSON, one object per refresh cycle. pipe to `jq` or other tools.

**non-TTY**: when stdout is not a terminal (piped), prints a single snapshot and exits.

## examples

```bash
# watch top 10 by memory, refresh every second
proc watch --sort mem --limit 10 -n 1

# watch node processes in current project
proc watch node --in .

# stream to jq
proc watch node --json | jq '.processes[].name'

# single snapshot when piped
proc watch node | head -20
```

## see also

[list](list) for a one-time snapshot, [by](by) for filtered listing.
