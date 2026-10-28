---
layout: page
title: Changelog
description: "Release history for proc."
permalink: /changelog
---

## 2.0.0 — 2026-10-27

This is a major release. Read **Changed** first: a name now matches the whole process name (add `--cmd` for the old matching), and `--json` no longer counts as `--yes`. The library API changed too (see the entries marked **Library API**).

**Changed**

- **BREAKING: a name matches the whole process name, not any part of the name or command line.** `proc kill node` used to kill every process whose name or full command line contained `node`, ignoring case. On one Mac a dry run matched 193 processes, among them `Claude Helper` and `Obsidian Helper (Renderer)`; with `--yes` all of them would have been killed. A name now equals the process name, the file name of its executable, or the file name of its first argument, ignoring case, as `killall` and `pkill` read it. The executable and first argument count because Linux cuts a process name to 15 characters. On Windows a trailing `.exe` is optional on either side. The same rule applies to every name target and every `--by` filter. A path is not a name: use `--cmd`, or `proc for <file>` for an executable. An empty name used to match every process and now matches nothing.
  **Migration:** add `--cmd` to keep the old matching (`proc kill node --cmd`), or name the process exactly. A script that passed a part of a name now gets exit 2 (`No process found matching`) where it matched before; one that relied on a command-line match needs `--cmd`
- **`--json` no longer counts as a yes.** Commands that change processes (`kill`, `stop`, `free`, `freeze`, `thaw`, `unstick`, `orphans --kill`, `stuck --kill`) acted without asking when given `--json`. They now need `--yes` like everywhere else: without it they exit 4 and change nothing, and so does running without a terminal. `--dry-run` still previews without `--yes`. With `--json` the error carries `needs_confirmation: true` and the `processes` the command would have changed, so a caller can show the list before retrying with `--yes`
- **On macOS the JSON `address` of an IPv6 listener loses its brackets**: `[::1]` is now `::1`, matching Linux and Windows
- **Action JSON lists the targets that weren't found.** Every command that takes targets puts `not_found` in its JSON, an array that is empty when every target was found. The found processes are still handled, so `success` and the exit codes are unchanged. `proc free` on a port nobody holds is success (exit 0) with the port in `not_found`
- **"Not supported on this OS" is exit 4.** `freeze` and `thaw` on Windows exited 1, while `stop --signal` exited 4. All three now exit 4
- **`orphans` is not supported on Windows: it exits 4.** It always reported "No orphaned processes found" there, because Windows has no PID 1 and does not re-parent a process whose parent exited. It now exits 4, like `freeze` and `thaw`. Linux and macOS are unchanged
- **`--version` says when a build isn't a release.** A build from `main` printed the version of the last release. A build from the release tag prints the plain version (`proc 2.0.0`); any other build in a git checkout prints `proc 2.0.0-dev (main@5e94647, 3 commits after v2.0.0)`. Builds without git (crates.io, a Docker context without `.git`) print the plain version
- **Library API.** Name matching takes a `proc_cli::core::NameMatch` (`Exact` or `Command`) as a last argument in `Process::find_by_name`, `resolve_target`, `resolve_targets`, `resolve_targets_excluding_self`, `matches_by_filter` and `apply_filters`; `resolve_targets` returns a `Resolved { processes, not_found, hints }`. `Process` has two new fields, `argv0` and `raw`. `ProcError` has new variants `NameNotFound`, `FilteredOut` and `ConfirmationRequired`; it is not `#[non_exhaustive]`, so an exhaustive `match` must handle them. `ProcError::NotSupported` exits 4. `PortInfo::find_by_port` is now `PortInfo::holders_of` and returns every holder. New `proc_cli::ui::must_confirm`; `Printer::ask_confirm` no longer treats `--json` as a yes. The command structs changed their fields (`target` is a `Vec<String>`, `ports`' `range` is parsed, new `cmd` and `wide`), and `Process::kill_and_wait` returns `Result<()>`. `proc_cli::cli` and `proc_cli::version` are new modules. See the [full changelog](https://github.com/yazeed/proc-cli/blob/main/CHANGELOG.md) for the list
- **Help and docs examples never pair `--yes` with a name.** `--yes` appears only with a `:port` or a PID; a name is shown with `--dry-run`, or without `--yes` so it asks first. A test reads the docs and fails on a name next to `--yes`. proc has no wildcards: an unquoted `node*` is expanded by your shell, and `--cmd` is the way to a partial match
- **CI fails when a line or a flag has no test.** The coverage gate now fails the build below 100% line coverage of `src/`, merged across Linux, macOS and Windows, and when a long or short flag of a process command has no test. The gate's report also prints each OS's own total, for information only: the merged 100% stays the only rule
- **The agent skill is rewritten for 2.0.0** around a safety model: what only reads, what can be undone and what ends a process; ranked rules (a process is yours only if you started it, a name is a search and never approval, preview with `--dry-run --json` and show the user, `--yes` is the user's consent to one list); and what to do on exit 4. It asks before touching what it didn't start. New `evals.md` lists twelve tasks with what the agent must and must never do
- **A bare `proc` orients you** instead of listing all 19 commands: the idea that a target is the same everywhere, and four commands that run exactly as printed
- **One process is always one line.** The process table truncates to the terminal width instead of wrapping, long directories keep their root and their tail, and `proc watch` uses the same layout
- **Website redesign ([proc-cli.com](https://proc-cli.com)).** A new layout and type, light and dark themes, an interactive command reader on the home page, a sidebar of commands, and a changelog sidebar of versions. Page titles, headings and tables use sentence case
- **`proc wait` notices an exit within a quarter second**, instead of checking only every `--interval`. `--interval` now sets how often the status line is printed
- **`proc stop` asks every process at once and waits for them together**, so several processes share one `--timeout`
- **`proc ports` and `proc on :port` show full process names on macOS** (lsof cut them to 9 characters)
- **`proc stuck` needs a process to stay stuck.** It must have run longer than `--timeout` and, in two readings a second apart, use over 50% CPU or be blocked in uninterruptible sleep. `stuck` now takes about 2 seconds
- **Port lookups on Linux need nothing installed.** proc reads the kernel's socket tables in `/proc` instead of running `ss`, so they work on minimal systems without iproute2, and the Docker image no longer installs it

**Added**

- **Several targets can be separate arguments.** `proc kill 1234 5678 :8080` works everywhere proc took a comma-separated list: `kill`, `stop`, `freeze`, `thaw`, `free`, `on`, `why`, `wait`, `unstick`, `watch` and the names of `list`. Separate arguments and comma lists mix, and a process named twice is handled once. `--in` takes the next word as its directory unless it starts with `-`, so put it after the targets or write `--in=DIR`
- **`--cmd` matches names the old way.** `proc kill serve --cmd` matches any process with `serve` in its name or full command line, like `pkill -f`. It is on every command that takes a name target or `--by`, and has no short form. When a name matches nothing but some command lines have it, the not-found message says so
- **`--wide` / `-w` on `list`, `by`, `in`, `for`, `stuck`, `orphans` and `watch`**: prints directories and arguments in full instead of fitting them to the terminal
- **[FAQ page](https://proc-cli.com/faq)**: 16 answers to the most searched process and port questions, each with the proc command and the classic alternative
- **Blog post**: [What's new in proc](https://proc-cli.com/posts/whats-new-in-proc)
- **Structured data and a favicon** on the website: SoftwareApplication, FAQPage, BreadcrumbList, post authors, social card images, and `favicon.ico`, `favicon.svg` and an `apple-touch-icon.png`

**Deprecated**

- **Comma-separated targets** (`proc kill :3000,:8080`). They still work throughout 2.x, and a command that uses one now prints a single `note:` on stderr naming the fix, built from your own targets: `note: comma-separated targets are deprecated and will be removed in a future major release; pass them as separate arguments: proc kill :3000 :8080`. It appears in human output only, so `--json` output is unchanged on stdout and stderr, and it is coloured on a terminal unless `NO_COLOR` is set. Separate arguments are the recommended form and commas will be removed in a future major release

**Removed**

- **Printing helpers and library functions nothing called**: `Printer::error`, `Printer::print_ports`, `Printer::print_port_info`, `Printer::print_kill_result`, `resolve_target_single` and `Process::wait`
- **Unused dependencies `tokio`, `anyhow` and `indicatif`**; builds are smaller and faster

**Fixed**

- **`proc free` called a port free when a copy of the holder took it back a moment later.** A port now counts as free only when it reads free in two looks a quarter of a second apart
- **A port held by several processes was handled as if one held it** (for example `127.0.0.1` and `[::1]`, or a master and its forked workers). A `:port` now means every process listening on it, in `kill`, `stop`, `free`, `on`, `info`, `wait`, `tree`, `watch` and `why`. `on :3000 --json` keeps `process` as the first holder and adds `processes`
- **Windows never listed IPv6 listeners, and macOS reported them differently.** All three systems now report `::1` and `::` (thanks @gtwatts, #1)
- **`proc kill --graceful` reported success on Windows when nothing happened**, and **`proc unstick --force` on Windows reported an already-gone process as failed**
- **`proc stuck` never found a stuck process on Windows**, and targeted `unstick` never saw one as stuck. CPU is now measured the way `list` and `info` do on every OS
- **`proc tree <name>` and `proc watch <name>` listed the shell that launched them.** Name matching now skips proc's ancestors in every command
- **`--in` found nothing when the path was a symlink or not in the OS's own form** (`/tmp` on macOS, 8.3 short names on Windows)
- **`proc stuck --timeout N` needed a process to be N+1 seconds old**, and **`proc for <file>` found nothing on Windows**
- **Not-found messages no longer nest a sentence inside a sentence or put the filter note inside the quotes**: `No process found matching 'node' (no matches with specified filters)`
- **proc could crash on a non-ASCII path, process name or argument.** Truncation now counts display columns and never splits a character
- **`proc stop`, `orphans --kill` and `stuck --kill` could hang forever** on a zombie whose parent didn't reap it; waiting after SIGKILL is limited to 5 seconds
- **Zombies counted as running on Linux and were never listed on macOS**
- **A reused PID counted as the original process**: `wait`, `stop` and the others now compare the start time too
- **`wait --timeout`, `free --wait` and `watch --interval` edge cases**: `--timeout 1` took 5 seconds, `--wait` wasn't a maximum, and `nan`, `inf`, a negative number or `0` crashed or busy-looped `watch`; they now exit 4 or are bounded
- **`proc ports --range` ignored a range it couldn't read**, and **a mistyped port such as `:70000` was searched for as a name**; both now exit 4
- **IPv6 addresses on Linux came out as `]`**, so `ports --exposed` left out ports listening on `::`
- **`proc unstick <target>` never tried**, and **CPU readings were wrong** (a process using a whole core showed as about 0.4%). `list`, `by`, `in`, `for`, `info`, `on`, `tree`, `orphans` and `wait -v` now measure CPU over 0.2 s before they show, filter or sort by it
- **Name matching read escaped text in some places and the raw text in others**, so `--by` could drop a process `find_by_name` found

**Security**

- **Signals can't reach a process that reused the PID.** Every signal first checks that the PID still belongs to the same process, by its start time
- **Process names from lsof can't inject terminal escapes or crash proc (macOS)**
- **Process data can't control the terminal.** Names, command lines, working directories, executable paths and users could carry escape sequences that recolour, retitle or rewrite the terminal of whoever ran `proc list`, `info`, `ports` or `watch`. Control characters are now shown as visible escapes, in every view and in JSON

## 1.14.5 — 2026-10-25

**Changed**

- **Uptime in every process view.** It was missing from several:
  - `proc on <PID|name>` shows an `Uptime:` line, like `proc on :port`
  - `proc tree` (including `--ancestors`) and `proc why` show `up 3m 12s` on each line, so a long-lived parent such as a forgotten tmux session stands out
  - `proc watch` has the UPTIME column on terminals at least 100 columns wide, and an `uptime:` line with `--verbose`
  - in JSON, `uptime_secs` is now on `tree` and `tree --ancestors` nodes, on `why`'s process and ancestry entries, and on `unstick`'s results; every other command already had it

  The JSON changes only add fields
- **`proc for` uses the standard layouts.** One process gets the same detail view as `proc on :port` (directory, path, listening ports, resources, uptime). Several get the standard process table (directory, arguments, uptime, status) followed by the ports they listen on, instead of a table of their own
- **`proc on :port`** reads uptime from `uptime_secs`, like every other command, instead of working it out from `start_time`
- **npm package: no install script.** The binary is now downloaded the first time you run `proc`, which is already what happened on npm 12 and newer (they skip install scripts). Its checksum is verified either way. `node install.js` in the package directory still downloads it ahead of time, for example in an image that will run offline. This removes the "install scripts" supply-chain alert that scanners such as Socket raise
- **Socket badge** in the README
- **Website: an [agent skill](https://proc-cli.com/skill) page.** It covers installing the skill (`npx skills add`, `bunx`, the Claude Code plugin), updating it, what it teaches agents, and prompts to try. It's linked from the navigation, the install page and the README
- **Demo recording** re-rendered, so `proc why` shows the uptime of each process in the chain

**Fixed**

- **Uptime of about 56 years for other users' processes on macOS.** proc can't read their start time, and the system reports it as 0, so `start_time` was `0`, `uptime_secs` was roughly the time since 1970, and `--min-uptime` always matched them. On one Mac that was 233 of 1,516 processes, including `launchd`. Their start time and uptime are now reported as unknown: left out of JSON, `-` in tables, and never matched by `--min-uptime`
- **`--status` with an unknown value matched every process**, so a typo such as `--status stoped` listed everything. It now exits 4 and lists the valid values (`running`, `sleeping`, `stopped`, `zombie`); `stop`, `sleep` and any letter case still work. The same filter is shared by `list`, `by`, `in`, `for` and `tree`, which each had their own copy
- **Windows: `proc stop --signal` ignored the signal** and sent an ordinary stop. Windows has no signals, so `--signal` now exits 4 there with an explanation

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

[Full changelog on GitHub](https://github.com/yazeed/proc-cli/blob/main/CHANGELOG.md), including unreleased changes
