---
layout: page
title: proc watch
description: "Real-time process monitoring."
permalink: /commands/watch
---

Live-updating process table. Aliases: `w`, `top`.

```bash
proc watch                 # watch all processes
proc top                   # same thing
proc watch node            # watch node processes
proc watch :3000           # watch process on port
proc watch :3000 :8080 node  # watch several targets together
```

Several targets are separate arguments. Comma-separated lists (`:3000,:8080`) still work but are deprecated.

Press `q`, `Esc`, or `Ctrl+C` to exit. Terminal is always restored cleanly.

The table has the same columns as `proc list`, including uptime on terminals at least 100 columns wide.

## Options

| Flag | Short | Description |
|------|-------|-------------|
| `--interval <secs>` | `-n` | Refresh interval (default: 2) |
| `--sort <key>` | `-s` | Sort by: cpu, mem, pid, name (default: cpu) |
| `--limit <n>` | `-l` | Limit results shown |
| `--in [<dir>]` | `-i` | Filter by directory. Takes the next word as its directory unless it starts with `-` (bare `--in` is the current directory, `--in=DIR` is explicit) |
| `--by <name>` | `-b` | Filter by process name (the whole name, ignoring case) |
| `--cmd` | | Match name targets and `--by` as a substring of the process name or full command line (like `pkill -f`) |
| `--min-cpu <n>` | | Minimum CPU % |
| `--min-mem <n>` | | Minimum memory (MB) |
| `--json` | `-j` | NDJSON output (one object per refresh) |
| `--verbose` | `-v` | Show extra details |
| `--wide` | `-w` | Print full directories and arguments instead of fitting the terminal |

## Modes

**Terminal (default)**: Alternate screen with auto-refresh. Adapts to terminal size.

**JSON** (`--json`): Newline-delimited JSON, one object per refresh cycle. Pipe to `jq` or other tools.

**Non-TTY**: When stdout is not a terminal (piped), prints a single snapshot and exits.

## Examples

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

## See also

[list](list) for a one-time snapshot, [by](by) for filtered listing.
