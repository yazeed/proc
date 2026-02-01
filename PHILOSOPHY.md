# Philosophy

**Simplicity is the ultimate sophistication.**

proc exists because process management on the command line is unnecessarily complex. We believe the answer to "what's using port 3000?" should be one command, not a Stack Overflow search.

## Core Belief

The best tool is the one you don't have to think about. When you reach for proc, the command you guess should be the command that works.

## Principles

### Semantic
Commands mean what they say. `kill` kills. `stop` stops gracefully. `on` tells you what's on a port. There are no surprises, no hidden behaviors, no flags that fundamentally change what a command does.

### Explicit
User intent must be clear. Destructive actions require confirmation or explicit flags. We never assume what you meant—if there's ambiguity, we ask. Silent failures are bugs.

### Complete
Cover the full workflow, nothing more. proc handles process and port management completely. It doesn't creep into service management, container orchestration, or monitoring dashboards. Scope discipline is a feature.

### Fast
Sub-100ms for all operations. Speed isn't a nice-to-have—it's table stakes. A slow CLI tool trains users to avoid it. proc should feel instantaneous.

### Obvious
If you have to read the docs, we failed. The command structure should be discoverable through tab completion and `--help`. Examples should cover real use cases, not contrived demos.

## Values

### Unified Targets
`:port`, `PID`, and `name` work the same way everywhere. Learn the pattern once, use it everywhere. No command-specific syntax, no special cases.

```bash
proc on :3000      # port
proc on 1234       # PID
proc on node       # name
proc kill :3000    # same pattern
proc info node     # same pattern
```

### Natural Grammar
Nouns observe state. Verbs change state.

**Nouns** (discovery): `ports`, `list`, `info`, `tree`, `on`, `by`, `in`
**Verbs** (action): `kill`, `stop`, `unstick`

You don't "run ports"—you look at ports. You don't "observe kill"—you kill.

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

5. **Is it something you'd use weekly?**
   Occasional needs can use existing tools. proc is for repeated friction.

6. **Does it follow our conventions?**
   Unified targets, natural grammar, consistent output. No exceptions.

If the answer to any question is no, the feature doesn't belong in proc.

## What proc Is Not

- **Not a service manager** — Use systemd, launchd, or supervisord
- **Not a container tool** — Use docker or podman
- **Not for remote processes** — Use ssh + proc
- **Not a monitoring system** — Use proper observability tools
- **Not a GUI** — proc is a CLI, intentionally

## The Goal

When someone asks "how do I find what's using port 3000?", the answer should be:

```bash
proc on :3000
```

Not a paragraph of explanation. Not a pipe chain. Not "it depends on your OS."

One command. Obvious. Done.

---

*"Perfection is achieved, not when there is nothing more to add, but when there is nothing left to take away."* — Antoine de Saint-Exupéry
