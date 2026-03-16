---
layout: page
title: commands
description: "complete reference for all proc commands."
permalink: /commands
---

## discovery

| command | aliases | description |
|---------|---------|-------------|
| [on](on) | `:` | port/process lookup (bidirectional) |
| [why](why) | | trace why a port is busy or show process ancestry |
| [for](for) | `f` | find processes by file path |
| [by](by) | `b` | filter processes by name |
| [in](in) | | filter processes by working directory |
| [list](list) | `l`, `ps` | list processes |
| [info](info) | `i` | show detailed process information |
| [ports](ports) | `p` | list all listening ports |
| [tree](tree) | `t` | show process tree |
| [watch](watch) | `w`, `top` | real-time process monitoring |
| [stuck](stuck) | `x` | find stuck/hung processes |
| [orphans](orphans) | `o` | find orphaned processes |

## lifecycle

| command | aliases | description |
|---------|---------|-------------|
| [kill](kill) | `k` | force kill processes (SIGKILL) |
| [stop](stop) | `s` | graceful stop with --signal support (SIGTERM, then SIGKILL) |
| [freeze](freeze) | | pause processes (SIGSTOP) |
| [thaw](thaw) | | resume frozen processes (SIGCONT) |
| [free](free) | | free ports by killing the occupying process |
| [unstick](unstick) | `u` | attempt to recover stuck processes |

## common options

most commands support these flags (check individual command pages for details):

| flag | short | description |
|------|-------|-------------|
| `--json` | `-j` | output as JSON |
| `--verbose` | `-v` | show extra details |
| `--in <dir>` | `-i` | filter by directory |
| `--by <name>` | `-b` | filter by process name |
| `--help` | `-h` | show help |

## target syntax

targets work the same way everywhere:

```bash
proc on :3000          # port
proc on 1234           # PID
proc on node           # name
proc kill :3000,:8080  # multiple (comma-separated)
```
