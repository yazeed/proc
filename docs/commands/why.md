---
layout: page
title: proc why
description: "trace why a port is busy or show process ancestry."
permalink: /commands/why
---

explain why a port is in use or trace the ancestry of a process. read-only — no confirmation needed.

```bash
proc why :3000     # trace why port 3000 is busy
proc why node      # show ancestry of node processes
proc why 1234      # show ancestry of PID 1234
```

## options

| flag | short | description |
|------|-------|-------------|
| `--json` | `-j` | output as JSON |
| `--verbose` | `-v` | show extra details |

## examples

```bash
# find out why port 3000 is busy
proc why :3000

# trace ancestry of node processes
proc why node

# trace ancestry of a specific PID
proc why 1234

# output as JSON for scripting
proc why :3000 --json
```

## see also

[on](on) for port/process lookup, [tree](tree) with `--ancestors` for full process tree.
