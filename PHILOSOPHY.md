# Philosophy

**Simplicity is the ultimate sophistication.**

proc exists because process management on the command line is unnecessarily complex. We believe the answer to "what's using port 3000?" should be one command, not a Stack Overflow search.

## Core Belief

The best tool is the one you don't have to think about. When you reach for proc, the command you guess should be the command that works.

## Principles

### Semantic
Commands mean what they say. `kill` kills. `stop` stops gracefully. `freeze` pauses and `thaw` resumes. `free` frees a port and checks that it's free. `on` tells you what's on a port, and `why` tells you why it's there. There are no surprises and no hidden behaviors. Flags refine a command rather than replace it; the two that add an action, `--kill` on `orphans` and `stuck`, follow the same confirmation rules as `kill` itself.

### Explicit
User intent must be clear. Destructive actions ask for confirmation (`--yes` skips it) and can be previewed with `--dry-run`. proc never guesses what a target means: a leading `:` is a port, a number is a PID, anything else is a name. Every failure has a non-zero exit code and a message that says what went wrong. Silent failures are bugs.

### Complete
Cover the full workflow, nothing more. proc handles process and port management completely: finding, inspecting, tracing, pausing, stopping, freeing and waiting. It doesn't creep into service management, container orchestration, or monitoring dashboards. Scope discipline is a feature.

### Fast
Speed isn't a nice-to-have; a slow CLI trains users to avoid it. proc answers in about a tenth of a second, process startup included, with no daemon and no index to keep warm.

### Obvious
If you have to read the docs, we failed. The command structure should be discoverable through tab completion, `--help` and the man page. Examples should cover real use cases, not contrived demos.

## Values

### Unified Targets
`:port`, `PID`, and `name` work the same way everywhere, and so do comma-separated lists of them. Learn the pattern once, use it everywhere.

```bash
proc on :3000          # port
proc on 1234           # PID
proc on node           # name
proc kill :3000,node   # same pattern, several targets
proc info node         # same pattern
```

The few commands that take something else say so in their name: `free` takes ports, `for` takes a file, `in` takes a directory. The `--in <dir>` and `--by <name>` filters narrow nearly every command the same way.

### Natural Grammar
Nouns observe state. Verbs change state.

**Discovery**: `on`, `for`, `by`, `in`, `list`, `info`, `ports`, `tree`, `why`, `orphans`, `stuck`
**Action**: `kill`, `stop`, `freeze`, `thaw`, `free`, `unstick`
**Monitoring**: `watch`, `wait`

You don't "run ports"—you look at ports. You don't "observe kill"—you kill.

### One Contract for People, Scripts and Agents
Every process and port command speaks JSON with `--json`: one document (`watch` streams one per refresh), with `action` and `success`, and exit codes that always mean the same thing (0 success, 1 error, 2 not found, 3 permission denied, 4 invalid input). A shell script, a CI job and an AI agent read proc the same way a person does, without parsing tables.

### Practical Simplicity
Every feature solves a real, repeated problem. We don't add features because they're technically interesting or because a competitor has them. We add features when users hit the same friction repeatedly.

### Easy to Remember
Consistent patterns mean knowing one command teaches you all commands. The cognitive load of proc should be near zero after the first use.

## The Feature Test

Before any feature enters proc, it must pass all six questions:

1. **Does it fit "process and port management"?**
   If it's about containers, services, or remote systems—it's out of scope.

2. **Can it be expressed in one obvious command?**
   If it requires a tutorial, it's too complex.

3. **Does it make the common case effortless?**
   Edge cases can require flags. The 80% case should be flag-free.

4. **Is user intent explicit?**
   Destructive operations need confirmation. Ambiguous inputs need clarification.

5. **Does it deepen proc's command of its domain?**
   Robust tools handle the case you didn't expect to need. Sophistication is completeness without complexity.

6. **Does it follow our conventions?**
   Unified targets, natural grammar, consistent output. No exceptions.

If the answer to any question is no, the feature doesn't belong in proc.

## What proc Is Not

- **Not a service manager** — Use systemd, launchd, or supervisord
- **Not a container tool** — Use docker or podman
- **Not for remote processes** — Use ssh + proc
- **Not a monitoring system** — Use proper observability tools; `proc watch` shows the present, not history
- **Not a GUI or interactive TUI** — proc is a CLI, intentionally: you say what you want, it does it

## The Goal

When someone asks "how do I find what's using port 3000?", the answer should be:

```bash
proc on :3000
```

Not a paragraph of explanation. Not a pipe chain. Not "it depends on your OS."

One command. Obvious. Done.

---

*"Perfection is achieved, not when there is nothing more to add, but when there is nothing left to take away."* — Antoine de Saint-Exupéry
