# Roadmap

proc follows a simple philosophy: **simplicity is the ultimate sophistication**.

The goal is not to accumulate features, but to cover the process and port management landscape completely, with commands that feel obvious. We seek sophistication in its simplest form.

See [PHILOSOPHY.md](PHILOSOPHY.md) for our full manifesto.

## Current Release (v1.5.0)

The core commands are complete, with the Proc Query Language, shell completions, file lookup, and full CI/CD automation:

| Area | Commands | Status |
|------|----------|--------|
| Discovery | `on`, `for`, `by`, `in`, `ports`, `list`, `info`, `tree`, `stuck` | ✅ |
| Lifecycle | `kill`, `stop`, `unstick` (all support multi-target) | ✅ |
| Tooling | `completions`, `manpage` | ✅ |

### v1.5.0 Highlights

- **`proc for <file>`**: Find processes by file path
  - `proc for ./script.py` — What's running this file?
  - `proc for /usr/bin/node` — Processes running this executable
  - `proc for app.log` — What has this file open?
  - Supports relative paths, absolute paths, and tilde expansion
  - Shows process info AND listening ports

### v1.4.x Highlights

- **Shell completions**: `proc completions bash|zsh|fish`
- **Man page generation**: `proc manpage`
- **Dry-run for stop**: `proc stop node --dry-run`

### v1.3.x Highlights

- **Proc Query Language**: Composable process discovery
  - `proc by node --in .` — Filter by name with directory
  - `proc in . --by node` — Filter by directory with name
  - `proc on node --in .` — Bidirectional lookup with filters
- **Multi-target support**: `proc kill :3000,:8080,node`
- **PID deduplication**: Overlapping targets resolved safely
- **Automated publishing**: All package managers update on release
  - crates.io, npm, Homebrew, Scoop, Docker — all via CI

## Planned

### v1.6 — Watch

Real-time monitoring for when you need to observe.

- [ ] `proc watch :3000` — Monitor a port
- [ ] `proc watch node` — Monitor processes by name
- [ ] `proc watch 1234` — Monitor a specific PID

**Competitive context:** procs offers `--watch` mode for continuous updates. proc's version will use unified targets.

## Under Consideration

Features that have valid use cases but are not yet prioritized. Each is evaluated against our [philosophy](PHILOSOPHY.md).

### Hog (Resource-Heavy Processes)

A dedicated command for finding resource hogs.

```
proc hog           # Find resource-heavy processes
proc hog --cpu     # Sort by CPU
proc hog --mem     # Sort by memory
```

**Use cases:**
- Quickly find what's consuming CPU/memory
- Shorter than `proc list --min-cpu 10 --sort cpu`

**Philosophy check:** ✅ Fits process management, ✅ one obvious command, ✅ common case effortless, ⚠️ functionality exists via flags.

**Status:** Functionality already exists in `list` via `--min-cpu`, `--min-mem`, and `--sort` flags. Would consider adding `hog` as a shorthand if there's user demand.

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

**Philosophy check:** ✅ Fits process management, ✅ obvious commands, ✅ explicit intent, ⚠️ niche (monthly, not weekly use).

**Status:** Useful but niche. Would consider if there's user demand.

### Quiet Mode

Suppress non-essential output for scripting.

```
proc kill :3000 -q    # Kill silently, only output errors
proc on :3000 -q      # Output PID only, no formatting
```

**Use cases:**
- Shell scripts that parse output
- CI/CD pipelines where minimal output is preferred
- Chaining with other commands

**Philosophy check:** ✅ Fits process management, ✅ obvious flag, ✅ covers scripting workflow, ✅ weekly use in scripts.

**Status:** Natural complement to `--json`. Would consider if there's user demand.

**Competitive context:** fkill offers `--silent` for similar use cases.

## Not Planned

These are outside proc's scope. See [PHILOSOPHY.md](PHILOSOPHY.md) for why.

- **Service management** — Use systemd, launchd, or supervisord
- **Container management** — Use docker or podman (killport has `--mode container`, we don't)
- **Remote processes** — Use ssh + proc
- **Historical data** — Use proper monitoring tools
- **GUI/Dashboard** — proc is a CLI tool
- **Interactive TUI mode** — fkill offers fuzzy search UI; proc is for when you know what you want
- **Auto-updates** — Use your package manager

---

Have an idea? Open a [discussion](https://github.com/yazeed/proc/discussions).
