---
layout: page
title: proc on
description: "Bidirectional port/process lookup."
permalink: /commands/on
---

Port/process lookup — works both ways. Alias: `:`.

```bash
proc on :3000          # what's on port 3000?
proc on node           # what ports is node using?
proc on 1234           # what ports is PID 1234 using?
proc on :3000 :8080    # multiple targets
```

Several targets are separate arguments. Comma-separated lists (`:3000,:8080`) still work but are deprecated.

Every view shows the process's directory and uptime.

A port can be held by several processes at once (`127.0.0.1:3000` and `[::1]:3000` are different sockets). `proc on :3000` lists all of them, and `--json` has them in `processes`, with `process` still the first.

## Options

| Flag | Short | Description |
|------|-------|-------------|
| `--in [<dir>]` | `-i` | Filter by directory. Takes the next word as its directory unless it starts with `-` (bare `--in` is the current directory, `--in=DIR` is explicit) |
| `--by <name>` | `-b` | Filter by process name (the whole name, ignoring case) |
| `--cmd` | | Match name targets and `--by` as a substring of the process name or full command line (like `pkill -f`) |
| `--json` | `-j` | Output as JSON |
| `--verbose` | `-v` | Show full command line |

## Examples

```bash
# what's running on port 3000?
proc on :3000

# what ports is node using? (reverse lookup)
proc on node

# only node processes in current directory
proc on node --in .

# JSON output for scripting
proc on :3000 --json
```

## See also

[info](info) for detailed process information, [ports](ports) to list all listening ports.
