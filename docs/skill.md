---
layout: page
title: Agent skill
description: "Teach your coding agent to use proc: install the proc-cli skill for Claude Code, Codex, Cursor, Gemini CLI and others."
permalink: /skill
---

The proc-cli skill teaches coding agents (Claude Code, Codex, Cursor, Gemini CLI and others) when to reach for proc and how to use it safely. Instead of `lsof -ti :3000 | xargs kill -9`, your agent runs `proc free :3000`, reads structured JSON, and checks with you before touching processes it didn't start.

## Install

proc itself has to be installed first (see [install](install)). Then add the skill:

```bash
npx skills add yazeed/proc-cli
```

Or with bun:

```bash
bunx skills add yazeed/proc-cli
```

To pick the agent and where it's installed:

```bash
npx skills add yazeed/proc-cli --skill proc-cli -a claude-code -y   # one agent, this project
npx skills add yazeed/proc-cli --skill proc-cli -g                   # every project (user-level)
```

### Claude Code plugin

In Claude Code, you can install it as a plugin instead:

```text
/plugin marketplace add yazeed/proc-cli
/plugin install proc-cli@proc-cli
```

## Update

```bash
npx skills update
```

Installs always fetch the current skill from the repository, so a new install is up to date too. The skill states which proc version it describes, so your agent can compare it with `proc --version`.

## What your agent learns

- **When to use proc**: a busy port (`EADDRINUSE`, "address already in use"), a dev server that won't start, leftover dev servers, watchers and headless browsers, a slow machine, and cleaning up before it finishes a task
- **What it replaces**: `lsof`, `ps | grep`, `pgrep`, `pkill`, `kill`, `netstat`, `pstree` and `top`, each mapped to the proc command for the job
- **The contract**: `--json` on every command, one JSON document per command (`watch` streams one per refresh), and fixed exit codes (0 success, 1 error, 2 not found, 3 permission denied, 4 invalid input)
- **Playbooks**: freeing a busy port, stopping its own background servers by port or PID rather than by a copied name pattern, checking for leftovers before finishing, and clean restarts
- **Safety**: only touch processes it started or you named, preview with `--dry-run`, prefer `stop` over `kill`, and ask before using `sudo`
- **Gotchas**: `kill` and `stop` don't signal child processes, `:3000` is a port but `3000` is a PID, and an orphaned process isn't necessarily abandoned

## Try it

Once it's installed, ask your agent things like:

- "What's on port 3000?"
- "Port 3000 is taken, free it"
- "Why is port 8080 busy?"
- "What did you leave running in this project?"
- "My machine is slow, what's using the CPU?"
- "Wait for the build to finish, then start the server"

## Where it lives

- Source: [`skills/proc-cli/`](https://github.com/yazeed/proc-cli/tree/main/skills/proc-cli) in the repository ([`SKILL.md`](https://github.com/yazeed/proc-cli/blob/main/skills/proc-cli/SKILL.md), and [`reference.md`](https://github.com/yazeed/proc-cli/blob/main/skills/proc-cli/reference.md) for every JSON output)
- Listing: [skills.sh/yazeed/proc-cli/proc-cli](https://www.skills.sh/yazeed/proc-cli/proc-cli), where it passes the security audits from Gen Agent Trust Hub, Socket and Snyk
- Format: the [Agent Skills](https://agentskills.io) standard, so any agent that supports skills can use it
