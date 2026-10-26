---
layout: page
title: agent skill
description: "teach your coding agent to use proc: install the proc-cli skill for Claude Code, Codex, Cursor, Gemini CLI and others."
permalink: /skill
---

the proc-cli skill teaches coding agents (Claude Code, Codex, Cursor, Gemini CLI and others) when to reach for proc and how to use it safely. instead of `lsof -ti :3000 | xargs kill -9`, your agent runs `proc free :3000`, reads structured JSON, and checks with you before touching processes it didn't start.

## install

proc itself has to be installed first (see [install](install)). then add the skill:

```bash
npx skills add yazeed/proc-cli
```

or with bun:

```bash
bunx skills add yazeed/proc-cli
```

to pick the agent and where it's installed:

```bash
npx skills add yazeed/proc-cli --skill proc-cli -a claude-code -y   # one agent, this project
npx skills add yazeed/proc-cli --skill proc-cli -g                   # every project (user-level)
```

### claude code plugin

in Claude Code, you can install it as a plugin instead:

```text
/plugin marketplace add yazeed/proc-cli
/plugin install proc-cli@proc-cli
```

## update

```bash
npx skills update
```

installs always fetch the current skill from the repository, so a new install is up to date too. the skill states which proc version it describes, so your agent can compare it with `proc --version`.

## what your agent learns

- **when to use proc**: a busy port (`EADDRINUSE`, "address already in use"), a dev server that won't start, leftover dev servers, watchers and headless browsers, a slow machine, and cleaning up before it finishes a task
- **what it replaces**: `lsof`, `ps | grep`, `pgrep`, `pkill`, `kill`, `netstat`, `pstree` and `top`, each mapped to the proc command for the job
- **the contract**: `--json` on every command, one JSON document per command (`watch` streams one per refresh), and fixed exit codes (0 success, 1 error, 2 not found, 3 permission denied, 4 invalid input)
- **playbooks**: freeing a busy port, stopping its own background servers by port or PID rather than by a copied name pattern, checking for leftovers before finishing, and clean restarts
- **safety**: only touch processes it started or you named, preview with `--dry-run`, prefer `stop` over `kill`, and ask before using `sudo`
- **gotchas**: `kill` and `stop` don't signal child processes, `:3000` is a port but `3000` is a PID, and an orphaned process isn't necessarily abandoned

## try it

once it's installed, ask your agent things like:

- "what's on port 3000?"
- "port 3000 is taken, free it"
- "why is port 8080 busy?"
- "what did you leave running in this project?"
- "my machine is slow, what's using the CPU?"
- "wait for the build to finish, then start the server"

## where it lives

- source: [`skills/proc-cli/`](https://github.com/yazeed/proc-cli/tree/main/skills/proc-cli) in the repository ([`SKILL.md`](https://github.com/yazeed/proc-cli/blob/main/skills/proc-cli/SKILL.md), and [`reference.md`](https://github.com/yazeed/proc-cli/blob/main/skills/proc-cli/reference.md) for every JSON output)
- listing: [skills.sh/yazeed/proc-cli/proc-cli](https://www.skills.sh/yazeed/proc-cli/proc-cli), where it passes the security audits from Gen Agent Trust Hub, Socket and Snyk
- format: the [Agent Skills](https://agentskills.io) standard, so any agent that supports skills can use it
