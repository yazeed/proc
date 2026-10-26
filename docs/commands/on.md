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
proc on :3000,:8080    # multiple targets
```

Every view shows the process's directory and uptime.

## Options

| Flag | Short | Description |
|------|-------|-------------|
| `--in [<dir>]` | `-i` | Filter by directory |
| `--by <name>` | `-b` | Filter by process name |
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
