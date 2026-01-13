# Roadmap

proc follows a simple philosophy: **simplicity is the ultimate sophistication**.

The goal is not to accumulate features, but to cover the process and port management landscape completely, with commands that feel obvious. We seek sophistication in its simplest form.

## Principles

- **Semantic**: Commands mean what they say
- **Explicit**: User intent must be clear—destructive actions require explicit flags
- **Complete**: Cover the full workflow, nothing more
- **Fast**: Sub-100ms for all operations
- **Obvious**: If you have to read the docs, we failed

## Values

- **Unified targets**: `:port`, `PID`, and `name` work the same way everywhere
- **Natural grammar**: Nouns to observe (`ports`, `ps`, `info`), verbs to act (`kill`, `stop`)
- **Practical simplicity**: Every feature solves a real, repeated problem
- **Easy to remember**: Consistent patterns—know one command, know them all

## Current Release (v1.0.0)

The core commands are complete:

| Area | Commands | Status |
|------|----------|--------|
| Discovery | `on`, `ports`, `ps`, `info`, `tree`, `stuck` | ✅ |
| Lifecycle | `kill`, `stop`, `unstick` | ✅ |

## Planned

### v1.1 — Polish

Shell completions and documentation to make proc feel native.

- [ ] Shell completions (bash, zsh, fish)
- [ ] Man pages
- [ ] `--dry-run` for all destructive commands

### v1.2 — Watch

Real-time monitoring for when you need to observe.

- [ ] `proc watch :3000` — Monitor a port
- [ ] `proc watch node` — Monitor processes by name
- [ ] `proc watch 1234` — Monitor a specific PID

## Under Consideration

Features that have valid use cases but are not yet prioritized:

### Hog (Resource-Heavy Processes)

A dedicated command for finding resource hogs.

```
proc hog           # Find resource-heavy processes
proc hog --cpu     # Sort by CPU
proc hog --mem     # Sort by memory
```

**Use cases:**
- Quickly find what's consuming CPU/memory
- Shorter than `proc ps --min-cpu 10 --sort cpu`

**Status:** Functionality already exists in `ps` via `--min-cpu`, `--min-mem`, and `--sort` flags. Would consider adding `hog` as a shorthand if there's user demand.

### Freeze/Thaw (SIGSTOP/SIGCONT)

Temporarily pause and resume processes without terminating them.

```
proc freeze :3000     # Pause process on port 3000
proc thaw :3000       # Resume frozen process
```

**Use cases:**
- Pause resource-heavy processes temporarily
- Freeze long-running transfers (rsync) to free disk space, then resume
- Pause a process to attach debugger or investigate

**Status:** Researched, useful but niche. Would consider if there's user demand.

## Not Planned

These are outside proc's scope:

- **Service management** — Use systemd, launchd, or supervisord
- **Container management** — Use docker or podman
- **Remote processes** — Use ssh + proc
- **Historical data** — Use proper monitoring tools
- **GUI/Dashboard** — proc is a CLI tool

## Philosophy

**Simplicity is the ultimate sophistication.**

Every feature is evaluated against:

1. Does it fit "process and port management"?
2. Can it be expressed in one obvious command?
3. Does it make the common case effortless?
4. Is user intent explicit?
5. Is it something you'd use weekly?
6. Does it follow our conventions?

If the answer to any is no, it doesn't belong in proc.

---

Have an idea? Open a [discussion](https://github.com/yazeed/proc/discussions).
