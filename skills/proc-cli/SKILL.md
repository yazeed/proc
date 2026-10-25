---
name: proc-cli
description: |
  Manage system processes and ports using the proc CLI tool. Find, inspect, kill, stop, freeze, thaw, free, unstick, and wait for processes by port, PID, name, file, or working directory. Use when the user asks to check what's running on a port, kill a process, free a port, find orphaned or stuck processes, pause/resume processes, trace why a port is busy, wait for a process to finish, find what's running a file, list processes started from a project directory, or any process/port management task. Also use when the user mentions proc, ports, PIDs, SIGKILL, SIGTERM, SIGSTOP, or process management.
---

# proc CLI — Process Management Tool

proc is a semantic CLI for process management on macOS, Linux, and Windows. Every command supports structured JSON output.

## Critical Rules

1. **Always pass `--json`** on every command for structured output
2. **Always pass `--yes`** on destructive commands to skip interactive prompts
3. **Use `--dry-run`** before destructive actions to preview what would happen
4. **Check the exit code**: 0=success, 1=general error (including partial failures and `wait` timeouts), 2=not found, 3=permission denied, 4=invalid input. `"success": false` always comes with a non-zero exit code
5. **Errors with `--json`** produce `{"action":"<cmd>", "success":false, "error":"...", "exit_code":N}` on stdout — including invalid flags or arguments
6. **Every JSON response** is a single document (except the `watch` stream) with `action` (the command name, aliases resolved) and `success` fields
7. **Scope destructive commands** with `--in <dir>` or `--by <name>` when targeting by name, so you only touch the processes you mean

## Target Syntax

Most commands accept the same target format:

- `:3000` — process listening on port 3000
- `1234` — process with PID 1234
- `node` — processes matching name "node" (substring match on name and command line)
- `:3000,:8080,node` — comma-separated multiple targets, deduplicated by PID

Exceptions: `proc free` takes ports only, `proc for` takes a file path, `proc in` takes a directory.

## Commands

### Discovery (read-only, no --yes needed)

```bash
proc on :3000 --json              # what process is on port 3000?
proc on 1234 --json               # what ports does PID 1234 use?
proc on node --json               # what ports are node processes using?
proc for ./server.py --json       # what's running (or holding open) this file?
proc by node --json               # find processes by name
proc by node --in . --json        # ...started from the current directory
proc in . --json                  # all processes whose cwd is under this directory
proc in ~/project --by node --json
proc list --json                  # all processes
proc list --min-cpu 10 --json     # processes using >10% CPU
proc list --status stopped --json # frozen/stopped processes
proc info :3000,1234 --json       # inspect one or more targets: full details
proc ports --json                 # all listening ports
proc ports --exposed --json       # only ports bound to 0.0.0.0 / ::
proc ports --range 3000-3999 --json
proc why :3000 --json             # trace ancestry — why is this port busy?
proc tree node --json             # descendants of node processes
proc tree 1234 --ancestors --json # path from the top ancestor down to PID 1234
proc orphans --in . --json        # processes whose parent died (reparented to PID 1)
proc stuck --json                 # processes >50% CPU running longer than --timeout (default 300s)
```

To inspect a process, use `proc info` (aliases: `inspect`, `i`; JSON `action` is always `"info"`). For each target it reports the name, PID, command line, executable path, working directory, user, parent PID, status, CPU, memory, start time and uptime. `list`, `by` and `in` return the same process fields for many processes at once.

### Lifecycle (destructive — always use --yes --json)

```bash
proc kill :3000 --yes --json               # force kill (SIGKILL)
proc kill node --in . --graceful --yes --json  # SIGTERM instead of SIGKILL
proc stop node --yes --json                # SIGTERM, then SIGKILL after --timeout (default 10s)
proc stop nginx --signal HUP --yes --json  # send a different initial signal
proc freeze node --yes --json              # pause (SIGSTOP)
proc thaw node --yes --json                # resume (SIGCONT)
proc free :3000 --yes --json               # kill + verify port is free (--wait, default 10s)
proc free :3000,:8080 --yes --json         # free multiple ports
proc unstick 1234 --yes --json             # SIGCONT then SIGINT; no kill
proc unstick 1234 --force --yes --json     # ...then SIGTERM, SIGKILL if still stuck
```

### Monitoring

```bash
proc wait node --timeout 60 --json       # block until all node processes exit (max 60s)
proc wait :3000 -n 1 -q --timeout 30 --json  # poll every 1s, quiet
timeout 10 proc watch node --json        # NDJSON stream, one object per refresh (-n seconds)
```

### Preview before acting

```bash
proc kill node --in . --dry-run --json   # show what would be killed
proc free :3000 --dry-run --json         # show what would be freed
proc unstick --dry-run --json            # show what would be attempted
```

## Filters

Not every command takes every filter — check `proc <cmd> --help`.

| Filter | Commands |
|--------|----------|
| `--in [dir]` (`-i`, defaults to `.`) | all except `why`, `free`, `in` |
| `--by <name>` (`-b`) | all except `why`, `free`, `by` and `list` (those two take the name as an argument) |
| `--min-cpu <pct>`, `--min-mem <MB>` | `list`, `by`, `in`, `for`, `tree`, `watch` |
| `--status <s>` (running, sleeping, stopped, zombie) | `list`, `by`, `in`, `for`, `tree` |
| `--min-uptime <secs>` | `list`, `by`, `in`, `for`, `tree` |
| `--parent <pid>` | `list`, `by`, `in` |
| `--sort <key>` (cpu, mem, pid, name) | `list`, `by`, `in`, `for`, `watch` |
| `--sort <key>` (port, pid, name) | `ports` |
| `--limit <n>` (`-n`; `-l` on `watch`) | `list`, `by`, `in`, `for`, `ports`, `watch` |

## Output Schemas

For complete JSON output schemas and field definitions, see [reference.md](reference.md).

## Agent/Pipe Readiness

- **One-shot queries**: `on`, `for`, `by`, `in`, `list`, `info`, `ports`, `why`, `tree`, `orphans`, `stuck` — return immediately
- **Blocking**: `proc wait` — always pass `--timeout`
- **Streaming**: `proc watch --json` — NDJSON forever; bound it with `timeout N` or `| head -n N`. Without `--json` and without a TTY it prints one snapshot. Prefer `proc list` for a single snapshot
- **Destructive**: `kill`, `stop`, `freeze`, `thaw`, `free`, `unstick`, and `orphans`/`stuck` with `--kill` — always pass `--yes --json`

## Gotchas

- Port targets must start with `:` — `proc on 3000` looks up PID 3000, not port 3000.
- `proc free` only accepts port targets (`:3000`). Use `proc kill` for name/PID targets.
- Name matching is substring-based: `proc by node` matches "node", "nodejs", "nodemon". Narrow with `--in` or use PIDs.
- `proc by` takes one pattern; `proc list` takes several, comma-separated: `proc list 'git commit,pre-commit' --json`. Patterns are plain case-insensitive substrings matched against the name and the full command line, not regular expressions.
- Uptime is `uptime_secs` in the JSON (seconds). `start_time` is a Unix timestamp, not a duration.
- `--dry-run --json` prints `"dry_run": true` plus the `processes` that would be affected; nothing is changed.
- `proc orphans --kill` and `proc stuck --kill` report like `proc kill` (`succeeded`/`failed`), with `action` `"orphans"`/`"stuck"`.
- If any process fails in `kill`/`stop`/`freeze`/`thaw`/`orphans --kill`/`stuck --kill`, the exit code is 1 and `failed` lists why; the ones in `succeeded` were still acted on.
- `proc wait` on timeout prints its result with `timed_out: true` and exits 1.
- `--signal` is only on `proc stop`. Valid signals: HUP, INT, QUIT, ABRT, KILL, TERM, STOP, CONT, USR1, USR2 (with or without `SIG`, any case).
- `--graceful` is only on `proc kill`.
- Unknown flags exit 4; with `--json` the error is a JSON object on stdout, otherwise plain text on stderr.
- `freeze` and `thaw` are Unix only — on Windows they fail with exit 1.
- `proc stuck` is a heuristic (sustained high CPU), not a hang detector.

## Example Workflow

```bash
# 1. Check what's on port 3000
proc on :3000 --json

# 2. Find out why it's there
proc why :3000 --json

# 3. Preview freeing it
proc free :3000 --dry-run --json

# 4. Free the port
proc free :3000 --yes --json

# 5. Verify it's free (expect exit code 2 = not found)
proc on :3000 --json
```
