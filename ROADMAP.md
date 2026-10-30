# Roadmap

This file is about what might come next. Nothing in it is promised. An idea ships only after it passes the feature test in [PHILOSOPHY.md](PHILOSOPHY.md), and many won't. When something ships it leaves this file: [CHANGELOG.md](CHANGELOG.md) is the history.

Each candidate lists what it would look like, how it fares against the feature test so far, and what's still open.

## Decided: the next major release

### Remove comma-separated targets

`proc kill :3000,:8080` has been deprecated since 2.0.0 in favour of separate arguments (`proc kill :3000 :8080`), and prints a notice on stderr when used (human output only, so `--json` is unaffected). The next major release removes it, so there is one spelling for several targets.

**Open:** what a comma does once removed: an error that shows the separate-argument form, or treating the whole word as one name.

## Candidates

### One word for the preview and the receipt

A command that changes processes lists them as `processes` in its preview (`--dry-run --json`, and the `needs_confirmation` error) and as `succeeded` and `failed` in its receipt. A script that previews and then acts has to know both names for the same set of processes. The idea: settle on one word, or make the receipt carry the preview's `processes` too, so the list that was approved and the list that was changed read the same way.

**Feature test:** follows the JSON contract and adds keys without changing any. **Open:** whether to add a key to the receipt or rename one (a rename breaks 2.0 scripts, so only in a major release).

### Signal words, not numbers

`-9`, `-15` and `-HUP` are notation people copy without knowing what they mean. proc already takes signal names (`--signal KILL`), but rejects numbers: `proc stop 1234 --signal 9` is an error. The idea: accept the number people already type, act on it, and answer with the word (`9 is SIGKILL`), so a habit turns into knowledge.

**Feature test:** fits the domain; makes the common case effortless for people with `kill -9` in their fingers. **Open:** whether `proc kill -9 <target>` (today: "unexpected argument") should also be accepted, and how that reads next to `kill` always meaning SIGKILL.

### A verb for reloading

`proc stop nginx --signal HUP` sends HUP and then, like every `stop`, force-kills whatever is still running after `--timeout`, so it can't safely reload a config. Reloading is something people do, so it could be a verb:

```bash
proc reload nginx      # send SIGHUP, don't escalate, report who got it
```

**Feature test:** one obvious command, natural grammar (a verb that changes state). **Open:** which signal each common server reloads on, and whether `reload` should verify anything afterwards.

### Defaults that follow the risk

Every confirmation prompt defaults to no today. A prompt that always defaults to no trains people to type yes without reading. The idea: default to yes when the action can be undone (`freeze`, which `thaw` reverses) and to no when it can't (`kill`). `--json` and scripts are unaffected: they still need `--yes`.

**Feature test:** "is the safe path the short path?" (proposed below). **Open:** which commands count as recoverable (`stop` can escalate to SIGKILL).

### Act only on the process you saw

An agent records a PID, and acts on it later. Within one command proc already checks a process's start time between finding it and signalling it, but nothing checks it against what the caller saw before: if that process exited and the OS gave its PID to a new one, `proc kill <pid> --yes` ends the new one. Today the skill asks agents to record `start_time` and compare it themselves, and to guard with `--by` and `--in` from the preview. The idea: a flag that carries what was seen, such as `proc kill 1234 --started 1700000000 --yes`, so a PID with another start time matches nothing (exit 2), as the `--by` and `--in` guards do.

**Feature test:** explicit (the caller says which process it means), and it puts the safe case in one command. **Open:** the form (`--started <unix time>`, or a target such as `1234@1700000000`), whether `--dry-run --json` should print the exact command to run next, and how often `--by` and `--in` already catch it.

### `proc :3000`

A bare target as a shortcut for `proc on`: the shortest possible answer to "what's on port 3000?". Today it's "unrecognized subcommand".

**Feature test:** one obvious command for the most common question. **Open:** whether a bare name or PID should work too (`proc node`), and the conflict with a process named like a future command.

### Errors that name what you meant

When a guess is wrong, the error should say the right command. A mistyped subcommand or value already gets the closest names (`proc kil` says "did you mean 'kill'?"); these don't yet:

- `proc kill 3000 --dry-run`, with no process 3000 but a listener on port 3000, could ask "did you mean :3000?".
- Flags people reach for from other tools: an agent tester typed `proc kill --name worker` and got "unexpected argument '--name' found". The error could say "to filter by name, use --by". Likewise `--port` (a target is `:3000`) and `--pid` (a PID is a plain number).

**Feature test:** obvious, explicit (proc says which reading it took), and it keeps one spelling per intent. **Alternative considered:** hidden aliases (`--name` meaning `--by`). They fix the typo silently but give one flag two spellings and teach nothing, which "Easy to remember" argues against. **Open:** how far suggestions go before they become guesses, and which borrowed flags are common enough to name.

### Hog

```bash
proc hog           # resource-heavy processes
proc hog --cpu
proc hog --mem
```

A shorter `proc list --min-cpu 10 --sort cpu`. **Feature test:** fits, one command, but it adds a second spelling for something `list` already does, which goes against the proposed "one obvious spelling per intent" (below). **Open:** whether there is real demand.

### Quiet mode

```bash
proc kill :3000 -q    # errors only
proc on :3000 -q      # the PID only
```

For shell scripts and pipelines that want one value without JSON. **Feature test:** fits, obvious flag. **Open:** demand, and what each command prints in quiet mode.

### Doctor

```bash
proc doctor
```

One health check after a crash or a messy session: port conflicts, orphans, stuck processes, zombies, each with the command that fixes it. **Feature test:** fits, one command, but it aggregates existing commands rather than adding a capability. **Open:** what counts as a problem worth reporting.

### Orphans on Windows

`proc orphans` exits 4 on Windows, because an orphan there can't be told by a parent PID of 1: Windows has no PID 1 and keeps the dead parent's PID on the child. The idea: call a process an orphan when its parent PID is no longer running, or has been reused by a process that started after the child.

**Feature test:** fits the domain; keeps one command the same on every OS. **Open / risk:** normal Windows processes outlive their launcher (`explorer.exe`, services, installers, anything started through a shell that exits), so the list would be long and mostly wanted, and `--kill` could take wanted processes with it. It would need to be list-only, or limited by `--in`, before it is safe.

### UDP ports

`proc on`, `ports` and `free` only see TCP listeners. **Open:** how UDP sockets show up in each command without making TCP output noisier.

### `tree` children in start order

`tree` lists siblings by PID. PIDs wrap and get reused, so PID order isn't spawn order: on one Mac, 9-21% of adjacent siblings had the higher PID as the older process. A tree is read top to bottom as "what started what", so start time (PID as the tie-break) is the natural order.

**Feature test:** deepens proc's command of its domain. **Open:** whether `tree` alone should differ from `list --sort pid`, or a `tree --sort` flag is better than changing the default (a sort flag on a hierarchy only orders siblings, which is easy to misread).

### `--sort uptime`

```bash
proc by node --sort uptime    # the longest-running first: which duplicate is the stale one
proc in . --sort uptime
```

The column is already shown and `--min-uptime` already filters by it. On `list`, `by`, `in`, `for` and `watch`. **Feature test:** fits, one obvious flag value. **Open:** the direction (oldest first matches cpu and mem putting the most extreme first) and the name (`uptime` or `started`).

### Sorting `orphans` and `stuck`

Both print processes in PID order (81 orphans on one Mac), which says nothing about which to look at first. A `--sort` like `list`'s would. **Feature test:** fits, but it's another way to ask a `list` question; `list --parent 1` is not the orphan set, so it isn't redundant. **Open:** real demand. `--limit` must never limit what `--kill` acts on.

### One meaning for `-n`

`-n` is `--limit` in `list`, `by`, `in`, `for` and `ports`, but `--interval` in `watch` and `wait` (where the limit is `-l`). **Feature test:** "Easy to remember" says one spelling means one thing. **Open:** which side moves, and how to deprecate the other without breaking scripts (a major release).

### The human clock

How long someone who has never used proc takes to get a correct answer. Unlike the machine clock (time per command), nothing measures it yet. **Open:** how to measure it (first-run sessions, agent evals like `skills/proc-cli/evals.md`).

## Proposed changes to PHILOSOPHY.md

PHILOSOPHY.md is the criteria, so it changes slowly and on purpose. These changes are proposed and not adopted:

- **Language, not notation:** prefer words to signal numbers and other notation; accept the notation and answer with the word.
- **Safe by default:** the dangerous path must be longer to type than the safe one; prompts default by risk, not always to no.
- **Compatible with habits:** familiar spellings keep working, and proc answers in its own words.
- **One obvious spelling per intent**, as part of "Easy to remember".
- **Fast, two clocks:** the machine clock (with an honest exception for CPU readings, which need a real sampling interval) and the human clock.
- **Two new feature-test questions:** "Would you say it out loud?" and "Is the safe path the short path?"

## Not planned

These are outside proc's scope. See [PHILOSOPHY.md](PHILOSOPHY.md) for why.

- **Service management:** use systemd, launchd or supervisord
- **Container management:** use docker or podman
- **Remote processes:** use ssh + proc
- **Historical data:** use monitoring tools; `proc watch` shows the present
- **A GUI or an interactive TUI:** proc is for when you know what you want. `proc watch` refreshes a live view, but its only key is quit
- **Auto-updates:** use your package manager

---

Have an idea? Open a [discussion](https://github.com/yazeed/proc-cli/discussions).
