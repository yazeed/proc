---
layout: page
title: changelog
description: "release history for proc."
permalink: /changelog
---

## 1.14.4 — 2026-10-25

**Fixed**
- **Linux: threads were listed as processes.** `list`, `by`, `in`, `orphans`, `stuck`, `watch` and name targets listed every thread of a process as a separate entry, with the process's command line. A Node dev server, for example, showed up a dozen times, and a name target such as `proc kill node` resolved those extra entries too. Only processes are listed now. macOS and Windows were not affected
- **`proc orphans` and `proc stuck`** printed two headlines for one result ("⚠ Found 2 orphaned processes", then "✓ Found 2 processes"). They now print one

**Changed**
- **New demo recording** (README GIF and website video). It tells one story:
  - `npm run dev` fails because port 3000 is taken
  - `proc on`, `proc why` and `proc free` find the old tmux session holding the port and free it
  - `proc orphans` finds the leftovers, shows them as JSON, and cleans them up after a confirmation

  It's recorded in a staged project (`demo/`), so the recording tool's own processes no longer appear, and it's larger and easier to read. To regenerate it, run the two commands at the top of `demo.Dockerfile`
- **`.dockerignore`** keeps `target/` and other unused folders out of Docker build contexts

## 1.14.3 — 2026-10-25

**Changed**
- **Repository renamed to [`yazeed/proc-cli`](https://github.com/yazeed/proc-cli)**, matching the crate, the npm package, the agent skill and proc-cli.com. Old `github.com/yazeed/proc` links, git remotes and release downloads redirect, so existing installs keep working. Install commands now use the new name: `npx skills add yazeed/proc-cli`, `/plugin marketplace add yazeed/proc-cli`, `nix profile install github:yazeed/proc-cli`. The Homebrew command is unchanged (`brew install yazeed/proc/proc`)
- **Docker image published as `yazeed/proc-cli`.** Docker Hub can't rename repositories, so each release also pushes the same image to `yazeed/proc`, and `docker pull yazeed/proc` keeps getting updates

**Removed**
- **`Formula/proc.rb` and `pkg/scoop/proc.json`**: hand-maintained copies left over from before publishing was automated (frozen at 1.2.2 and 1.3.3 since January). Releases have written the real Homebrew formula and Scoop manifest to `yazeed/homebrew-proc` and `yazeed/scoop-bucket-proc` ever since; nothing read these

## 1.14.2 — 2026-10-25

**Fixed**
- **`proc stop --signal`** only checked the signal name after the dry run and the confirmation prompt. `--dry-run` reported success for a signal the real run rejects (`--signal 9`), and without `--yes` you confirmed before being told the name was invalid. An unknown signal now exits 4 straight away, with or without `--dry-run`

**Changed**
- **Website** moved to [proc-cli.com](https://proc-cli.com). The old `yazeed.github.io/proc` address redirects there
- **PHILOSOPHY.md** describes proc as it is today: every command in the grammar, the JSON and exit-code contract shared by people, scripts and agents, how targets are read without guessing, and `--kill` on `orphans`/`stuck`

## 1.14.1 — 2026-10-25

**Fixed**
- **`proc in <dir>`** exited 4 ("Directory not found") when the directory had been deleted, even with processes still running in it, such as a dev server left behind in a removed git worktree. It now lists them. A missing path with nothing running in it still exits 4, as a typo
- **Linux: processes in a deleted directory** reported their `cwd` as `<dir> (deleted)`, which the kernel appends, so `--in <dir>` never matched them. `cwd` now shows the plain path, as on macOS, and `--in` matches it

**Changed**
- **Skill**: tells agents when to reach for proc. It covers:
  - a fuller description of situations that trigger the skill (a busy port, `EADDRINUSE`, leftover dev servers and headless browsers, a slow machine)
  - a table of the shell commands proc replaces (`lsof`, `ps | grep`, `pkill`, `kill`)
  - playbooks for a busy port, cleaning up background work, checking for leftovers before finishing, a slow machine, and clean restarts
  - safety rules for not touching other people's or other agents' processes
  - how to check the installed version and install proc
  - the command aliases, and the flags it was missing (`list --path`, `ports --local`, `tree --depth`/`--compact`, `orphans --kill`)
  - new gotchas: `kill`/`stop` don't signal children, how to find processes left running in a deleted directory, and an orphan isn't necessarily abandoned
- **Release checklist**: triggers a skills.sh refresh after publishing, since skills.sh only re-reads the repo when someone installs the skill

## 1.14.0 — 2026-10-25

**Added**
- **Uptime in process output**: every process in JSON has `uptime_secs`; tables from `list`, `by`, `in` and the other process lists show an `UPTIME` column on terminals at least 100 columns wide, and `--verbose` prints it
- **`proc inspect`**: alias of `proc info`, next to `i`
- **Skill**: explains that inspecting a process means `proc info` (or `proc inspect`) and what it reports, that `by` takes one pattern while `list` takes comma-separated ones, and that `start_time` is a timestamp while `uptime_secs` is the duration

**Changed**
- **Release checklist**: after publishing, upgrades the local Homebrew `proc` on its own (`brew upgrade yazeed/proc/proc`) and checks its version

## 1.13.4 — 2026-10-25

**Fixed**
- **Linux binaries** failed to start on anything older than Ubuntu 24.04 (`GLIBC_2.39 not found`, e.g. on Debian 12), because they were built on the `ubuntu-latest` runner and linked against its glibc. They are now static musl builds (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`) with no glibc dependency. Tested on x86_64 and ARM64 with Amazon Linux 2, Debian 10, 11 and 12, Ubuntu 20.04 and Alpine 3.20. File names are unchanged, so the curl install, npm, Homebrew on Linux and cargo-binstall pick them up as before
- **Docker image**: `on :port`, `ports` and `free` failed with `Failed to run ss: No such file or directory`, because the image didn't include `ss`, which port lookups use on Linux. The image now installs `iproute2`. The documented `docker run` command adds `--net=host`, which port lookups need to see the host's ports
- **`user`** showed the numeric user ID (e.g. `"501"`) in JSON, `info`, `list` and `watch`. It now shows the user name, and falls back to the ID when the ID has no name on the system. Scripts that read `user` as a number need updating

**Changed**
- **CI** builds the same Linux targets as a release, including ARM64, so a broken release build shows up on every push instead of at tag time
- **cargo-binstall**: musl hosts (Alpine) now resolve to the Linux release files
- **Release checklist**: runs the published Linux binaries on Debian 11 and Alpine

## 1.13.3 — 2026-10-24

**Fixed**
- **`npm install proc-cli`** failed for every version with `Redirect to untrusted host: release-assets.githubusercontent.com`. GitHub now serves release downloads from that host, which the installer's allowlist didn't include. It is now allowed; the existing hosts and the SHA-256 checksum check are unchanged
- **npm 12**: install scripts no longer run by default, so the binary was never downloaded and `proc` crashed. The `proc` command now downloads and verifies the binary on first run when it is missing, with progress on stderr so `--json` output stays clean. If the package directory isn't writable, the binary goes to `~/.cache/proc-cli/<version>` (`%LOCALAPPDATA%\proc-cli\<version>` on Windows). A failed download during install no longer fails `npm install`; it is retried on first run
- **npm wrapper**: a `proc` killed by a signal now exits non-zero, and a binary that can't be started reports an error instead of crashing

**Changed**
- **Release checklist**: installs the npm package from a local tarball before tagging, and runs a real `npm install` after publishing

## 1.13.2 — 2026-10-24

**Changed**
- **npm publishing** uses Trusted Publishing (OIDC) from the release workflow instead of a stored token. The `NPM_TOKEN` secret is no longer used. The publish job runs on Node 24 with npm 11.5.1 or newer, as Trusted Publishing requires. 1.13.0 and 1.13.1 were not published to npm, so npm goes from 1.12.3 to 1.13.2

## 1.13.1 — 2026-10-24

**Fixed**
- **Docker image**: the binary failed to start (`GLIBC_2.39 not found`) on both amd64 and arm64, since at least 1.12.3. The build stage used `rust:latest`, which moved to Debian trixie, while the runtime stage is `debian:bookworm-slim` (glibc 2.36). The build stage now uses `rust:1-bookworm`
- **Release workflow**: the crates.io and npm publish steps reported success even when publishing failed, because the result was piped through `tee` without `pipefail`. This hid that 1.13.0 was never published to npm

## 1.13.0 — 2026-10-24

A consistent JSON and exit-code contract: every command prints one JSON document, `action` is always the command name, and `"success": false` always comes with a non-zero exit code. Scripts that relied on the old behaviour should check the **Changed** entries below.

**Changed**
- **`--dry-run --json`** now reports the command's own `action` (e.g. `"kill"`) and `"dry_run": true`, instead of a list result with `"action": "list"` (`kill`, `stop`, `freeze`, `thaw`, `free`, `orphans --kill`, `stuck --kill`)
- **`proc orphans --kill` / `proc stuck --kill` with `--json`** print one action result (`"action": "orphans"`/`"stuck"`, `succeeded`/`failed`) instead of two JSON documents
- **`proc thaw --json`** reports `"action": "thaw"` instead of `"resume"`
- **`proc why` / `proc tree` JSON** use lowercase `status` (`"running"`) and `protocol` (`"tcp"`), matching the Process and Port objects
- **Invalid flags and arguments** exit with code 4 (invalid input) instead of 2, which meant "not found"; with `--json` the error is a JSON object on stdout
- **`proc wait --json` on timeout** exits 1, as it already did without `--json`
- **`proc stop`, `freeze`, `thaw`, `orphans --kill`, `stuck --kill`** exit 1 when any process fails, as `proc kill` already did
- **`proc free`** exits 1 when a port is still in use after `--wait`
- **`proc unstick`** exits 1 when a process is still stuck or failed; an explicit target removed by `--in`/`--by` is "not found" (exit 2)
- **`proc info`** exits 2 when no target is found
- **`proc on` with multiple targets and `--json`** prints one document (`count`, `results` with `target`/`process`/`ports`, `not_found`) instead of one per target; exits 2 when no target is found
- **`proc in <path>`** exits 4 when the directory does not exist, like `proc for` with a missing file

**Fixed**
- **`proc why --json`** prints a JSON error and exits 2 when nothing is found, instead of printing nothing and exiting 0; multiple targets print one document instead of one per target
- **`proc kill --json`** no longer prints a second JSON error document after a partial failure
- **Error `action`** uses the canonical command name when an alias is used (`proc k` → `"kill"`)
- **JSON errors** are detected for short-flag clusters such as `-yj`
- **Human output**: "Stopped" instead of "Stoped"
- **Agent skill**: `SKILL.md` covers `proc for`, `proc in`, `proc unstick`, `--ancestors`, `--graceful`, `--exposed`/`--local`/`--range`, `--status`, `--min-uptime`, `--parent`, with a per-command filter table and the bounded `proc watch --json` stream; `reference.md` adds the `on <PID>`, `tree --ancestors`, `--dry-run`, `unstick`, and `watch` shapes
- **Plugin manifest**: `.claude-plugin/marketplace.json` version kept in sync and added to the release checklist

**Added**
- **Docs**: agent skill install instructions (`npx skills add yazeed/proc`, Claude Code plugin) in README and the install page; exit code table in README and the commands page
- **Tests**: JSON contract integration tests (`tests/cli_json.rs`)

## 1.12.3 — 2026-05-03

**Fixed**
- **Clippy**: Use `sort_by_key` instead of `sort_by` for name sorting (Rust 1.95+ lint)

## 1.12.2 — 2026-03-30

**Security**
- **npm install script hardened**: SHA-256 checksum verification, redirect host pinning, `execFileSync` (no shell injection), dead code removed

**Fixed**
- **README**: SafeSkill badge, correct badge ordering
- **Skills**: Added `proc for` and empty result JSON schemas to reference
- **Rust docs**: Added `wait`, `free`, `why`, `freeze/thaw`, `--signal`, `--json` to Quick Start and Features

## 1.12.1 — 2026-03-27

**Fixed**
- **CI**: Upgrade to `softprops/action-gh-release@v2` — v1 fails with immutable releases enabled

## 1.12.0 — 2026-03-27

**Added**
- **`proc wait`**: Block until process(es) exit — the pipe-friendly replacement for `while ps aux | grep ... | grep -v grep; do sleep; done`
  - `proc wait node` — wait until all node processes exit
  - `proc wait :3000 --timeout 60` — wait up to 60s for port process to exit
  - `proc wait node -n 10 -q` — check every 10s, quiet mode for scripting
  - Supports `--in`, `--by`, `--json`, `--verbose` like other commands
  - Reports each process as it exits, with elapsed time
  - JSON output includes per-process exit timing and timeout status

**Fixed**
- **`--by` now matches full command line**: The `--by` filter across all commands now matches against both process name AND command line arguments (consistent with how positional targets and `proc by` work). Previously `proc wait node --by KXBTCD` would not match a process whose args contained "KXBTCD" — now it does. Affects: kill, stop, freeze, thaw, info, tree, stuck, unstick, orphans, wait, on, watch, for
- **JSON output for empty results**: Commands that find nothing (`stuck`, `orphans`, `free` when ports already free) now output `{"action":"...", "success":true, "count":0, "message":"..."}` in JSON mode instead of empty stdout
- **Consistent JSON envelopes**: `on` (name target), `for`, `why` now wrap results in `{"action":"...", "success":true, "count":N, "results":[...]}` instead of outputting bare arrays
- **Consistent JSON action field**: All commands now use lowercase action names (`"kill"`, `"stop"`, `"freeze"`, `"resume"`) instead of mixed-case past-tense verbs (`"Killed"`, `"Stopped"`)
- **Command-specific action names**: `by`, `in`, `stuck`, `orphans` now report their own command name as the JSON `action` field instead of generic `"list"`
- **`wait` timeout no longer double-outputs JSON**: Previously emitted both a WaitOutput and an error envelope; now emits a single `{"success":false, "timed_out":true, ...}` object
- **Error envelope includes `action` field**: JSON error responses now include the command name (e.g. `{"action":"kill", "success":false, "error":"...", "exit_code":2}`)
- **`watch` JSON includes `success` field**: Consistent with all other commands
- **`tree --ancestors` uses `action: "tree"`**: Consistent regardless of `--ancestors` flag
- **Ancestor chain exclusion**: Name matching now excludes the entire ancestor process chain (up to 10 levels) instead of just the immediate parent. Prevents false matches in deep shell environments (IDE → terminal → shell → shell wrapper → proc)

**Changed**
- **Shared `matches_by_filter`**: Extracted the `--by` matching logic into `core::filters::matches_by_filter()`, replacing 14 inline copies with a single consistent implementation
- **Agent Skills**: Added `proc wait` to skill reference, added "Agent/Pipe Readiness" section documenting which commands are safe for agents vs interactive-only
- **`src/lib.rs`**: Clarified `watch` as interactive TUI, `wait` as pipe-friendly blocking

**Updated**
- Updated dependencies (`mio`, `roff`, `unicode-segmentation`, `wasm-bindgen`)

## 1.11.0 — 2026-03-23

**Changed**
- **CLAUDE.md**: Added explicit rules requiring user approval before every push, tag, and release. Added mandatory RELEASE.md checklist compliance for all versions including patches

**Updated**
- Updated dependencies (`itoa`, `unicode-segmentation`)

## 1.10.1 — 2026-03-23

**Fixed**
- **Parent shell self-matching**: Name-based searches (`proc by`, `proc on <name>`, `proc tree <name>`) no longer match the parent shell process whose command line contains proc's own arguments. Previously, `proc on :99999 --json` would match the shell running proc instead of returning "not found". Now correctly returns exit code 2 with a JSON error

## 1.10.0 — 2026-03-23

**Added**
- **JSON error envelope**: With `--json`, errors now output `{"success":false, "error":"...", "exit_code":N}` to stdout instead of plain text to stderr — enables reliable machine parsing by LLMs and scripts
- **Agent Skills**: `skills/proc-cli/SKILL.md` — Claude Code skill for process management with proc. Includes structured command reference, JSON output schemas, and LLM usage guidelines. Compatible with the [Agent Skills](https://agentskills.io) open standard

## 1.9.1 — 2026-03-16

**Fixed**
- **Windows build**: Gate unix-only imports behind `#[cfg(unix)]` in `freeze.rs` and `thaw.rs` — fixes unused import errors with `-D warnings` on Windows CI

## 1.9.0 — 2026-03-16

**Added**
- **`proc freeze`**: Pause processes with SIGSTOP
  - `proc freeze node` — freeze all node processes
  - `proc freeze :3000 --yes` — freeze by port, skip confirmation
  - Supports `--in`, `--by`, `--dry-run`, `--json`, `--verbose`
- **`proc thaw`**: Resume frozen processes with SIGCONT
  - `proc thaw node` — resume all frozen node processes
  - `proc thaw :3000` — resume process on port 3000
  - Supports `--in`, `--by`, `--dry-run`, `--json`, `--verbose`
- **`proc orphans`**: Find orphaned processes (parent exited, reparented to init/launchd)
  - `proc orphans` — list orphaned processes
  - `proc orphans --in .` — orphans in current directory
  - `proc orphans --kill --yes` — find and kill orphans
  - Filters out system daemons on both macOS and Linux
- **`proc why`**: Trace why a port is busy or show process ancestry
  - `proc why :3000` — ancestry chain with port context
  - `proc why node` — how was this node process started?
  - Shows working directory and command for the target process
  - Full JSON output support
- **`proc free`**: Free ports by killing the process and verifying availability
  - `proc free :3000` — kill process, verify port freed
  - `proc free :3000,:8080 --yes` — free multiple ports
  - `proc free :3000 --wait 30` — wait up to 30s for port to free
  - Reports per-port success/failure (handles TIME_WAIT)
  - Port-only targets; rejects PIDs/names with clear error message
- **`--signal` flag on `proc stop`**: Send custom initial signal instead of SIGTERM
  - `proc stop nginx --signal HUP` — reload config (SIGHUP)
  - `proc stop worker --signal USR1` — application-defined signal
  - Accepts signal names: HUP, INT, QUIT, ABRT, KILL, TERM, STOP, CONT, USR1, USR2

**Changed**
- **Core**: Added `Process::send_signal()` for arbitrary Unix signal delivery
- **Core**: Added `Process::find_orphans()` for orphan detection with system process filtering

**Updated**
- Updated dependencies (`clap`, `sysinfo`, `libc`, `tempfile`, and others)

## 1.8.1 — 2026-03-06

**Fixed**
- README demo image: use absolute URL so it renders on Docker Hub, crates.io, and npm

**Added**
- GitHub Pages blog with 7 technical posts
- Install page with all platforms, shell completions, and man page setup
- Blog redesign: terminal editorial theme with syntax highlighting and SEO meta tags

## 1.8.0 — 2026-03-06

**Added**
- `proc watch`: real-time process monitoring with auto-refresh
  - aliases: `proc top`, `proc w`
  - configurable interval (`-n`), sort (`-s`), limit (`-l`)
  - combines with `--in`, `--by`, `--min-cpu`, `--min-mem`
  - alternate screen with clean terminal restore on exit
  - NDJSON output (`--json`) for streaming
  - non-TTY detection: single snapshot when piped

**Fixed**
- multi-target "not found" output consolidated into single warning

## 1.7.4 — 2026-03-01

**Fixed**
- self-exclusion: proc no longer shows itself in results

## 1.7.3 — 2026-02-26

**Added**
- working directory in output: see which project folder a process runs from

**Changed**
- table `PATH` column replaced with `DIR` (working directory)

---

[full changelog on github](https://github.com/yazeed/proc-cli/blob/main/CHANGELOG.md)
