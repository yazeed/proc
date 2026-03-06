# Roadmap

proc follows a simple philosophy: **simplicity is the ultimate sophistication**.

The goal is not to accumulate features, but to cover the process and port management landscape completely, with commands that feel obvious. We seek sophistication in its simplest form.

See [PHILOSOPHY.md](PHILOSOPHY.md) for our full manifesto.

## Current Release (v1.8.0)

The core commands are complete, with real-time monitoring, the Proc Query Language, shell completions, file lookup, consistent filtering, terminal-adaptive tables, and full CI/CD automation:

| Area | Commands | Status |
|------|----------|--------|
| Discovery | `on`, `for`, `by`, `in`, `ports`, `list`, `info`, `tree`, `stuck` | ✅ |
| Monitoring | `watch` (alias: `top`) — real-time process monitoring | ✅ |
| Lifecycle | `kill`, `stop`, `unstick` (all support multi-target + filters) | ✅ |
| Tooling | `completions`, `manpage` | ✅ |

### v1.8.0 Highlights

- **`proc watch`**: Real-time process monitoring with auto-refresh
  - `proc watch` — watch all processes (alias: `proc top`)
  - `proc watch node` — watch node processes
  - `proc watch :3000` — watch process on port 3000
  - `--interval/-n` — configurable refresh interval (default: 2s)
  - `--sort/-s` — sort by cpu, mem, pid, name
  - `--limit/-l` — cap number of results
  - Combines with `--in`, `--by`, `--min-cpu`, `--min-mem` filters
  - Alternate screen + raw mode for clean terminal experience
  - NDJSON output (`--json`) for streaming to other tools
  - Non-TTY detection: single snapshot when piped

### v1.7.4 Highlights

- **Self-exclusion**: `proc` no longer shows itself in `find_all()` results — fixes false positives across `list`, `by`, `in`, `stuck`, and other enumeration commands

### v1.7.3 Highlights

- **Working directory in output**: `proc on`, `proc info`, and all table views now show the process working directory — instantly tells you which project folder a process is running from
- **Table `PATH` → `DIR` column**: Process tables now show working directory instead of executable path

### v1.7.0 Highlights

- **Terminal-adaptive tables**: Process and port tables now adapt to terminal width using `comfy-table`, no more overflow on 80-column terminals
- **Deduplication**: Extracted shared utilities (`resolve_in_dir`, `format_duration`, `truncate_string`, `colorize_status`) — 12 duplicate copies eliminated
- **Flag consistency**: `--verbose`/`-v` added to `stop`, `tree`, `unstick`; `--json` short flag standardized to `-j` across all commands; `--in` defaults fixed on `on` and `for`
- **New filters**:
  - `--min-uptime` on `list`, `by`, `in`, `for`, `tree` — filter by process uptime
  - `--parent` on `list`, `by`, `in` — filter by parent PID
  - `--range` on `ports` — filter by port range (e.g., `3000-9000`)
  - `--limit` on `ports` and `for` — cap result count
  - `--sort` on `for` — sort by cpu, mem, pid, name
  - `--dry-run` on `stuck --kill` — preview before killing
- **Multi-target `proc list`**: `proc list node,python` — comma-separated names, deduplicated
- **Unified confirmation prompts**: All destructive commands use consistent `⚠` icon

### v1.6.0 Highlights

- **`--in` and `--by` everywhere**: All commands now support `--in` (directory) and `--by` (name) filters
  - `proc kill node --in .` — Only kill node processes in current directory
  - `proc stop node --by worker` — Only stop "worker" node processes
  - `proc ports --by node --in .` — Ports from node processes in cwd
  - `proc stuck --by node` — Only stuck node processes
- **`proc ports`**: Renamed `--filter` to `--by` for consistency

### v1.5.x Highlights

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

**Philosophy check:** ✅ Fits process management, ✅ obvious commands, ✅ explicit intent, ✅ deepens domain mastery (pause/resume completes the process lifecycle).

**Status:** Strong candidate. Completes the process lifecycle alongside kill/stop/unstick.

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

### Not Planned

These are outside proc's scope. See [PHILOSOPHY.md](PHILOSOPHY.md) for why.

- **Service management** — Use systemd, launchd, or supervisord
- **Container management** — Use docker or podman (killport has `--mode container`, we don't)
- **Remote processes** — Use ssh + proc
- **Historical data** — Use proper monitoring tools
- **GUI/Dashboard** — proc is a CLI tool
- **Interactive TUI mode** — fkill offers fuzzy search UI; proc is for when you know what you want. Note: `proc watch` provides real-time monitoring without interactivity.
- **Auto-updates** — Use your package manager

---

Have an idea? Open a [discussion](https://github.com/yazeed/proc/discussions).
