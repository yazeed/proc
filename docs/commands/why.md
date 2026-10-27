---
layout: page
title: proc why
description: "Trace why a port is busy or show process ancestry."
permalink: /commands/why
---

Explain why a port is in use or trace the ancestry of a process. Read-only — no confirmation needed.

```bash
proc why :3000     # trace why port 3000 is busy
proc why node      # show ancestry of node processes
proc why 1234      # show ancestry of PID 1234
proc why :3000 1234  # several targets, one answer each
```

Several targets are separate arguments. Comma-separated lists (`:3000,1234`) still work but are deprecated.

Each step of the chain shows how long that process has been running (`up 2d 4h`), so a long-lived parent such as a forgotten `tmux` session stands out; `--json` includes `uptime_secs`.

## Options

| Flag | Short | Description |
|------|-------|-------------|
| `--json` | `-j` | Output as JSON |
| `--verbose` | `-v` | Show extra details |

## Examples

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

## See also

[on](on) for port/process lookup, [tree](tree) with `--ancestors` for full process tree.
