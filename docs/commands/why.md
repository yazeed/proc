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

each step of the chain shows how long that process has been running (`up 2d 4h`), so a long-lived parent such as a forgotten `tmux` session stands out; `--json` includes `uptime_secs`.

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
