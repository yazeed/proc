---
layout: page
title: proc kill
description: "force kill processes."
permalink: /commands/kill
---

send SIGKILL to processes. asks for confirmation unless `--yes`. alias: `k`.

```bash
proc kill :3000            # kill process on port 3000
proc kill node             # kill all node processes
proc kill :3000,:8080,node # kill multiple targets
```

## options

| flag | short | description |
|------|-------|-------------|
| `--yes` | `-y` | skip confirmation |
| `--dry-run` | | show what would be killed |
| `--graceful` | `-g` | send SIGTERM instead of SIGKILL |
| `--in [<dir>]` | `-i` | filter by directory |
| `--by <name>` | `-b` | filter by process name |
| `--json` | `-j` | output as JSON |
| `--verbose` | `-v` | show extra details |

## examples

```bash
# preview before killing
proc kill node --dry-run

# kill without confirmation
proc kill :3000 --yes

# graceful shutdown (SIGTERM)
proc kill node --graceful

# only kill node in current project
proc kill node --in .

# kill multiple targets at once
proc kill :3000,:8080,node
```

## see also

[stop](stop) for graceful shutdown with timeout, [unstick](unstick) for stuck processes.
