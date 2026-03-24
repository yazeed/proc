---
name: proc-cli
description: |
  Manage system processes and ports using the proc CLI tool. Find, inspect, kill, stop, freeze, thaw, and free processes by port, PID, or name. Use when the user asks to check what's running on a port, kill a process, find orphaned processes, free a port, pause/resume processes, trace why a port is busy, or any process/port management task. Also use when the user mentions proc, ports, PIDs, SIGKILL, SIGTERM, SIGSTOP, or process management.
---

# proc CLI — Process Management Tool

proc is a semantic CLI for process management. It provides structured JSON output for all commands.

## Critical Rules

1. **Always pass `--json`** on every command for structured output
2. **Always pass `--yes`** on destructive commands to skip interactive prompts
3. **Use `--dry-run`** before destructive actions to preview what would happen
4. **Check exit code** first: 0=success, 2=not found, 3=permission denied, 4=invalid input
5. **Errors with `--json`** produce `{"success":false, "error":"...", "exit_code":N}` on stdout

## Target Syntax

All commands accept the same target format:

- `:3000` — process listening on port 3000
- `1234` — process with PID 1234
- `node` — processes matching name "node" (substring match on name and command line)
- `:3000,:8080,node` — comma-separated multiple targets

## Commands

### Discovery (read-only, no --yes needed)

```bash
proc on :3000 --json              # what process is on port 3000?
proc on node --json               # what ports are node processes using?
proc by node --json               # find processes by name (like ps aux | grep)
proc by node --in . --json        # filter by working directory
proc list --json                  # all processes
proc list --min-cpu 10 --json     # processes using >10% CPU
proc info :3000 --json            # detailed info for target
proc ports --json                 # all listening ports
proc why :3000 --json             # trace ancestry — why is this port busy?
proc orphans --json               # find orphaned processes
proc stuck --json                 # find stuck/hung processes
proc tree node --json             # process tree
```

### Lifecycle (destructive — always use --yes --json)

```bash
proc kill :3000 --yes --json            # force kill (SIGKILL)
proc stop node --yes --json             # graceful stop (SIGTERM then SIGKILL)
proc stop nginx --signal HUP --yes --json  # send custom signal
proc freeze node --yes --json           # pause (SIGSTOP)
proc thaw node --yes --json             # resume (SIGCONT)
proc free :3000 --yes --json            # kill + verify port freed
proc free :3000,:8080 --yes --json      # free multiple ports
```

### Preview before acting

```bash
proc kill node --dry-run --json   # show what would be killed
proc free :3000 --dry-run --json  # show what would be freed
```

## Common Filters

Combine with most commands:

| Filter | Example |
|--------|---------|
| `--in <dir>` | `--in .` or `--in /path/to/project` |
| `--by <name>` | `--by node` |
| `--min-cpu <n>` | `--min-cpu 10` |
| `--min-mem <n>` | `--min-mem 100` |
| `--sort <key>` | `--sort cpu` (keys: cpu, mem, pid, name) |
| `--limit <n>` | `--limit 10` |

## Output Schemas

For complete JSON output schemas and field definitions, see [reference.md](reference.md).

## Gotchas

- `proc free` only accepts port targets (`:3000`). Use `proc kill` for name/PID targets.
- `proc watch` is interactive — never use it from an agent. Use `proc list` or `proc by` instead.
- Name matching is substring-based: `proc by node` matches "node", "nodejs", "nodemon".
- Port targets must start with `:` — `proc on 3000` treats 3000 as a PID, not a port.
- `--signal` is only on `proc stop`. Valid signals: HUP, INT, QUIT, ABRT, KILL, TERM, STOP, CONT, USR1, USR2.
- `freeze` and `thaw` are Unix only — they return an error on Windows.

## Example Workflow

```bash
# 1. Check what's on port 3000
proc on :3000 --json

# 2. Preview freeing it
proc free :3000 --dry-run --json

# 3. Free the port
proc free :3000 --yes --json

# 4. Verify it's free (expect exit code 2 = not found)
proc on :3000 --json
```
