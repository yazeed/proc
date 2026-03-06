---
layout: page
title: proc on
description: "bidirectional port/process lookup."
permalink: /commands/on
---

port/process lookup — works both ways. alias: `:`.

```bash
proc on :3000          # what's on port 3000?
proc on node           # what ports is node using?
proc on 1234           # what ports is PID 1234 using?
proc on :3000,:8080    # multiple targets
```

## options

| flag | short | description |
|------|-------|-------------|
| `--in [<dir>]` | `-i` | filter by directory |
| `--by <name>` | `-b` | filter by process name |
| `--json` | `-j` | output as JSON |
| `--verbose` | `-v` | show full command line |

## examples

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

## see also

[info](info) for detailed process information, [ports](ports) to list all listening ports.
