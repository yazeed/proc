---
name: proc-cli
description: >-
  Finds, inspects and stops processes and ports with the proc CLI (macOS, Linux, Windows) under
  a safety protocol: preview, show the user, then act by PID. Use when a port is busy
  (EADDRINUSE, "address already in use"), to see what listens on a port, runs from a directory
  or holds a file, to kill, stop, free, pause, resume or wait for a process, to clean up the dev
  servers and headless browsers an agent started, or to look into a slow machine or a stuck
  process. Use it instead of lsof, ps, pgrep, pkill, killall, kill or netstat. A name can match
  processes of other projects and other agents, so anything the agent did not start needs a dry
  run and the user's approval first. Not for containers, system services or remote hosts.
---

# proc: find and stop processes and ports without hitting the wrong one

proc targets processes by port, PID, name, file or directory. It can kill, stop and freeze processes, so the risk is acting on one that is not yours. Read the safety model before any command that changes a process.

Run `proc --version`. This skill is for proc 2.0 or later. If a command here exits 4 with "unexpected argument", the installed proc is older: read "Older versions" in [reference.md](reference.md), or ask the user to upgrade. If proc is missing, ask the user before installing: `brew install yazeed/proc/proc`, `cargo install proc-cli`, `npm install -g proc-cli`, or `scoop bucket add proc https://github.com/yazeed/scoop-bucket-proc && scoop install proc`.

## Safety model

| Class | Commands | What to do |
|-------|----------|------------|
| Read-only | `on`, `for`, `by`, `in`, `list`, `info`, `ports`, `why`, `tree`, `watch`, `wait`, `stuck`, `orphans` | Run freely. |
| Reversible | `freeze`, `thaw` | Dry run first unless the process is yours. |
| Can end the process | `stop` (SIGTERM, then SIGKILL after `--timeout`), `unstick` (SIGINT ends most programs) | Dry run, then ask unless yours. |
| Irreversible | `kill` (SIGKILL), `free`, `orphans --kill`, `stuck --kill`, `unstick --force` | Dry run, then ask unless yours. Prefer `stop`. |

Rules, in priority order:

1. **A process is yours only if you started it in this session and recorded its PID** (or its port). Everything else needs the user's approval. Record at launch: `cmd & pid=$!`.
2. **A name is a search, not approval.** "Kill node" means "find node processes". On one developer laptop `proc kill node --dry-run` matched 185 processes across many projects, and `--in .` from a project directory still matched 10, all of them MCP tool servers of agent sessions. A matching name does not make a process yours.
3. **Preview anything that is not yours.** Run the same command with `--dry-run --json`. Read `count` and `processes` (`pid`, `name`, `cwd`, `command`, `parent_pid`, `uptime_secs`). Stop and ask if the count is higher than you expect or any `cwd` or parent is outside your work.
4. **Ask with specifics.** Show the user each process's PID, name, cwd, command, parent and uptime, say what the command will do to it, and wait for a yes. A PID or port the user named is approval only for that target, and only if the preview shows what they described. With nobody to ask, report instead of acting.
5. **`--yes` is the user's consent to one specific list. It is never a retry flag.** Pass it only on PID or port targets that are yours or were approved from a preview. Never pass it with a name.
6. **Exit 4 with `needs_confirmation: true` means nothing changed.** `processes` is what the command would have touched. Show it to the user. Do not re-run with `--yes` until rule 4 is met, and then re-run by the PIDs you showed, not by the original name.
7. **Act by PID and verify.** Add `--in=<dir> --by <name>` as a guard: if the PID was recycled or moved, the command finds nothing and exits 2. Afterwards confirm every PID is gone.

Never do these: `--yes` with a name; `orphans --kill` or `stuck --kill`; `kill` before `stop` has been tried; `sudo` without asking; targeting an ancestor of your own process (check `proc tree $$ --ancestors --json`); treating `--in .` as proof of ownership. proc skips itself and its ancestor shells only for name targets. A PID or port target is never protected.

## Targets

| Target | Meaning | Use |
|--------|---------|-----|
| `:3000` | every process listening on port 3000 (all holders) | Preferred. Port numbers always start with `:`. |
| `1234` | PID 1234 | Preferred. |
| `node` | processes whose whole name is `node` (case-insensitive), or whose executable or argv[0] file name is | Search only, with `--dry-run` and `--in=<dir>`. |
| `node --cmd` | a substring of the name or full command line, like `pkill -f` | Only after a dry run of the exact-name search found nothing or too little. Matches far more. |

Give several targets as separate arguments: `proc stop <pid> <pid> :<port> --dry-run --json`. Commas still work but are deprecated. A path is not a name: `proc kill ./server` finds nothing.

`--in` filters by working directory. It takes the next word as its value unless that starts with `-`, so put it after the targets or write `--in=<dir>`. It narrows a search; it does not prove ownership. Use a directory only you use, such as a temp dir or worktree you created.

## Read-only commands (one default per task)

| Task | Command |
|------|---------|
| What is on a port? | `proc on :<port> --json` (`processes` lists every holder) |
| All listening ports (TCP) | `proc ports --json`, `proc ports --range <from>-<to> --json` |
| What does a PID use or hold? | `proc info <pid> --json`, `proc on <pid> --json` |
| What runs in a directory? | `proc in <dir> --json`, add `--by <name>` |
| What has this file open? | `proc for <file> --json` |
| Where did it come from? | `proc why :<port> --json`, `proc tree <pid> --ancestors --json` |
| Children of a process | `proc tree <pid> --json` |
| Busiest processes | `proc list --sort cpu --limit 10 --json` |
| Wait for exit | `proc wait <pid> --timeout 60 --json` (always set `--timeout`) |

Other commands and every filter are in [reference.md](reference.md).

## Playbooks

### A port is busy

```bash
proc on :<port> --json                # who holds it: read processes[]
proc free :<port> --dry-run --json    # the list free would stop
```

If every holder is a process you started, run `proc free :<port> --yes --json`. Otherwise show the holders to the user and ask. Or start your server on another port. Afterwards `proc on :<port> --json` exits 2 when the port is free. `free` stops every holder of the port (IPv4 and IPv6 sockets, workers that share it) with SIGTERM, then SIGKILL after 5 seconds. If it returns `freed: false`, run `proc on :<port> --json` again and report who still holds it. Do not escalate with `kill -9` or `sudo`.

### Stop a server or job you started

```bash
proc stop <pid> --in=<dir> --by <name> --yes --json
proc info <pid> --json                # exit 2 means gone
```

`stop` and `kill` signal only the PIDs you target, not their children: stopping `npm run dev` can leave its `node` child running. List children with `proc tree <pid> --json` before you stop the parent, and stop each child you started. `stop` already escalates to SIGKILL after `--timeout`. Use `kill` only on a process of yours that survived `stop`.

### Stop something you did not start

```bash
proc kill <name> --in=<dir> --dry-run --json     # a search, not an action
proc stop <pid> <pid> --yes --json               # only after the user approves these PIDs
```

Run the first line with `stop` or `kill`, whichever the user asked for. Show the preview (rule 4). If the user approves, act by the PIDs you showed. If the preview is empty with exit 2 and the error says "N processes have '<name>' in their command line", show that hint to the user. Run `--cmd` only after a dry run of it.

### Clean up before you finish

Stop only the PIDs you recorded. Anything else that looks like a leftover (a listener in the project, a headless browser) may belong to the user or another agent: report it with its PID, cwd and parent, and do not stop it. Check with `proc ports --in <dir> --json`.

### The machine is slow or something looks stuck

```bash
proc list --sort cpu --limit 10 --json
proc stuck --json
```

Both are leads, not verdicts. `stuck` flags any process that has run over 300 seconds and, in two readings a second apart, uses over 50% of a core or is blocked in the kernel. On one laptop it listed a `ts-node` project and a `node` at 453% CPU: busy, not necessarily hung. Report each process's `cwd`, `uptime_secs`, `parent_pid` and `command` and let the user decide.

## After a command

| Exit | Meaning | Next step |
|------|---------|-----------|
| 0 | Done (or the dry run succeeded) | Check `not_found`: every target you passed must appear in `succeeded`, `processes` or `results`. |
| 1 | Partly failed, a port stayed busy, or `wait` timed out | Read `succeeded` and `failed`. Retry only the PIDs in `failed`. Report anything the OS refused. |
| 2 | Nothing matched | Do not widen the target. Re-run the read-only command. Add `--cmd` only after a dry run. |
| 3 | Permission denied | Stop and ask. Do not use `sudo` unprompted. |
| 4 | Invalid input, `needs_confirmation`, or not supported on this OS | Read `error`. For `needs_confirmation` follow rule 6. |

A signal the OS refuses (a process of another user) is reported in `failed` with exit 1, not as exit 3. Treat it as "this process is not mine": stop and ask.

With `--json` every command prints one JSON document on stdout, including errors: `{"action", "success": false, "error", "exit_code"}`. `success: false` always comes with a non-zero exit. Use the full schemas in [reference.md](reference.md) before you parse `kill`, `stop` or `free` output.

## Platform notes

- Windows: `freeze`, `thaw` and `stop --signal` exit 4 (not supported). `kill --graceful` fails for console programs, so use `stop`, which escalates after `--timeout`. `for` finds no open files. Names include `.exe`, which is optional when matching.
- macOS has no `timeout` command. Bound `proc watch` with `| head -n 5`, or use `proc list`.
- `ports` and `on` show TCP listeners only. IPv6 addresses read `::1` and `::`.
