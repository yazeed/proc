# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.7.1] - 2026-02-11

### Fixed

- **STATUS column wrapping**: Fixed table column too narrow for "Running"/"Sleeping" text
- **Memory display**: Human-readable adaptive units — `512KB`, `6.0MB`, `3.0GB` instead of always showing MB
- **PATH column too narrow**: Removed hardcoded 19-char truncation, increased minimum width so paths are readable
- **ARGS column showing executable**: When a process has no arguments, shows `-` instead of repeating the full executable path
- **Misleading error tips**: Replaced clap's confusing `tip: to pass '...' as a value, use '-- ...'` with smart "did you mean?" suggestions using edit distance and substring matching (e.g., `--cpu` → `did you mean '--min-cpu'?`)
- **Help example consistency**: Removed `-y` flag from kill example to match the pattern of other examples

## [1.7.0] - 2026-02-11

### Added

- **Terminal-adaptive tables**: Process and port tables now use `comfy-table` with `ContentArrangement::Dynamic` — tables adapt to terminal width automatically, no more overflow on 80-column terminals. Falls back to 120 chars when stdout is not a TTY
- **New filters**:
  - `--min-uptime <seconds>`: Filter by process uptime (on `list`, `by`, `in`, `for`, `tree`)
  - `--parent <pid>`: Filter by parent PID (on `list`, `by`, `in`)
  - `--range <start-end>`: Filter ports by range, e.g. `--range 3000-9000` (on `ports`)
  - `--limit/-n`: Limit result count (on `ports`, `for`)
  - `--sort/-s`: Sort results by cpu, mem, pid, or name (on `for`)
  - `--dry-run`: Preview before killing stuck processes (on `stuck --kill`)
- **Multi-target `proc list`**: Comma-separated names with PID deduplication
  - `proc list node,python` — find both node and python processes
- **`--verbose/-v`**: Added to `stop`, `tree`, and `unstick` (previously missing)
- **Shared utility modules**: `src/core/filters.rs` and `src/ui/format.rs`
- **Unified confirmation prompts**: `print_confirmation` and `print_action_result` in Printer

### Changed

- **`--json` short flag standardized**: All commands now use explicit `-j` (previously some used inferred short flags)
- **`--in` flag on `on` and `for`**: Now supports `num_args = 0..=1` with `default_missing_value = "."` matching all other commands
- **Confirmation prompts unified**: All destructive commands now use `⚠` icon consistently (stop previously used `!`)
- **Philosophy criterion #5 updated**: Changed from "Is it something you'd use weekly?" to "Does it deepen proc's command of its domain?"

### Removed

- **Duplicated code**: Eliminated 12 copies of `resolve_in_dir`, 3 copies of `format_duration`, 3 copies of `truncate_string`, and inline `colorize_status` — all replaced by shared modules

## [1.6.0] - 2026-02-09

### Added

- **`--in` and `--by` flags across all commands**: Consistent filtering by directory and process name
  - `proc kill node --in .` — Only kill node processes in current directory
  - `proc stop node --by worker` — Only stop node processes matching "worker"
  - `proc info node --in . --by server` — Scoped process info
  - `proc tree node --in .` — Tree scoped to directory (targeted mode only)
  - `proc stuck --by node` — Only stuck node processes
  - `proc unstick --in . --by node` — Scoped unstick
- **`--in` flag for `proc ports`**: Filter listening ports by process working directory
  - `proc ports --in .` — Only ports from processes in current directory

### Changed

- **`proc ports`**: Renamed `--filter`/`-f` to `--by`/`-b` for consistency with other commands

### Updated

- Updated dependencies (`clap`, `sysinfo`, `regex`, `anyhow`, and others)

## [1.5.1] - 2026-02-03

### Security

- Updated `bytes` dependency to 1.11.1 to fix integer overflow vulnerability (RUSTSEC-2026-0007)

### Changed

- Added `cargo audit` to release checklist

## [1.5.0] - 2026-02-02

### Added

- **`proc for <file>`**: Find processes by file path
  - Find processes running a specific executable: `proc for /usr/bin/node`
  - Find processes with a file open (via lsof): `proc for ./data.json`
  - Supports relative paths, absolute paths, and tilde expansion: `proc for ~/bin/myapp`
  - Shows process info AND listening ports (like `on` command)
  - Composable filters: `--in`, `--by`, `--min-cpu`, `--min-mem`, `--status`

### Removed

- **`--path` filter from `proc by`**: Use `proc for <file>` instead
- **`--exe-path` filter from `proc in`**: Use `proc for <file>` instead

## [1.4.2] - 2026-02-01

### Fixed

- **Self-matching bug**: `proc kill <target>` no longer kills itself when target matches its own command line arguments
- **Dry-run message visibility**: Message now appears after the process table (not before) so it's visible without scrolling
- **Kill feedback**: All killed processes now show in results (previously proc would die mid-loop if it matched itself)

### Changed

- Destructive commands (`kill`, `stop`, `unstick`) now exclude the current process from target resolution

## [1.4.1] - 2026-02-01

### Added

- **PHILOSOPHY.md**: Project manifesto documenting principles, values, and the feature test
- **RELEASE.md**: Comprehensive release checklist and publishing process
- **CLAUDE.md**: Project-specific instructions for Claude Code
- **Shell completions docs**: Added to README.md
- **Man page docs**: Added to README.md

### Changed

- **ROADMAP.md**: Updated with v1.4.0 highlights, competitive context, and philosophy references
- **src/lib.rs**: Updated crate docs for docs.rs with v1.3/v1.4 features

## [1.4.0] - 2026-01-29

### Added

- **Shell completions**: Generate completions for bash, zsh, and fish
  - `proc completions bash > /etc/bash_completion.d/proc`
  - `proc completions zsh > ~/.zsh/completions/_proc`
  - `proc completions fish > ~/.config/fish/completions/proc.fish`

- **Man page generation**: `proc manpage > /usr/local/share/man/man1/proc.1`

- **`--dry-run` for stop command**: Preview what would be stopped
  - `proc stop node --dry-run`

## [1.3.3] - 2026-01-29

### Changed

- Docs: Complete README rewrite for clarity and completeness
- Docs: Streamlined package READMEs (npm, Scoop, Homebrew) to link to main docs

## [1.3.2] - 2026-01-29

### Added

- **Scoop bucket**: `scoop bucket add proc https://github.com/yazeed/scoop-proc`
- CI: Automated Scoop bucket updates on release

### Fixed

- CI: npm publishing now uses automation token (NPM_TOKEN secret)

## [1.3.1] - 2026-01-29

### Changed

- CI: npm trusted publishing via OIDC (no long-lived tokens)
- CI: Automated Homebrew tap updates on release
- CI: Fixed ARM64 Linux cross-compilation
- CI: Updated Dockerfile to `rust:latest` for edition2024 support
- Docs: Improved README hero examples showcasing multi-target support
- Docs: Added npm badge to README

### Fixed

- npm package now includes README for better discoverability

## [1.3.0] - 2026-01-28

### Added

- **Proc Query Language**: Composable process discovery primitives
  - **`proc by <name>`** — Filter processes by name with all filters
    - Example: `proc by node --in . --min-cpu 5`
  - **`proc in <path>`** — Filter processes by working directory
    - Example: `proc in ~/projects --by python`
    - Supports `.`, relative paths, absolute paths, and `~` expansion

- **Multi-target support** for kill, stop, info, and on commands
  - Comma-separated targets: `proc kill :3000,:8080,node`
  - Mixed target types: ports (`:3000`), PIDs (`1234`), names (`node`)
  - PID deduplication prevents double-kills on overlapping targets
  - Single confirmation prompt shows all targets before action
  - Not-found targets show warnings but don't block found targets

- **Combinable filters** across commands
  - `proc on node --in .` — Node processes on ports, filtered by directory
  - `proc by node --in .` — Node processes in current directory
  - `proc in . --by node` — Same result, different entry point

- **Comprehensive help examples** covering all command variants and scenarios

### Changed

- Updated dependencies: colored 3.1, sysinfo 0.38, indicatif 0.18, nix 0.31

- `proc on` now supports `--in` filter for directory-based filtering
- `proc info` now supports comma-separated targets
- Help text reorganized into semantic sections (Discovery, Info, Kill, Stop, etc.)

## [1.2.3] - 2026-01-21

### Added

- **Tier 3 distribution channels**:
  - **Scoop** (Windows): `pkg/scoop/proc.json` manifest with auto-update
  - **AUR** (Arch Linux): `pkg/aur/PKGBUILD` for `yay -S proc`
  - **npm** wrapper: `pkg/npm/` for `npm install -g proc-cli`
  - **Nix flake**: `flake.nix` for `nix profile install github:yazeed/proc`

## [1.2.2] - 2026-01-21


### Added

- **cargo-binstall support**: `cargo binstall proc-cli` now downloads pre-built binaries
  - 10x faster than `cargo install` (no compilation required)
  - Configured for all platforms: macOS, Linux, Windows (Intel and ARM)
- **Docker Hub automation**: Images automatically published on release
  - Multi-arch support: linux/amd64, linux/arm64
  - Available at `docker run yazeed/proc`

## [1.2.1] - 2026-01-15

### Changed

- Updated dependencies: colored 3.1, sysinfo 0.38, indicatif 0.18, nix 0.31

- Enhanced `--version` output to include repository URL and license

## [1.2.0] - 2026-01-15

### Added

- **ARGS column** in `proc list` output showing command arguments
  - Displays script names for Python/Node processes (e.g., `daily_spread_trader.py`)
  - Simplifies paths to filenames for readability
  - Enables finding processes by script name: `proc list my_script.py`

### Changed

- Updated dependencies: colored 3.1, sysinfo 0.38, indicatif 0.18, nix 0.31

- Reorganized `proc list` columns: PID, PATH, NAME, ARGS, CPU%, MEM, STATUS
- Improved process identification for interpreted languages (Python, Node, Ruby, etc.)

## [1.1.0] - 2026-01-14

### Changed

- Updated dependencies: colored 3.1, sysinfo 0.38, indicatif 0.18, nix 0.31

- **Breaking:** Renamed `ps` command to `list` for better semantics
  - `ps` remains as an alias for backwards compatibility
- Updated tagline: "Semantic CLI tool for process management. Target by port, process id (PID), name or path."

## [1.0.2] - 2026-01-13

### Changed

- Updated dependencies: colored 3.1, sysinfo 0.38, indicatif 0.18, nix 0.31

- Streamlined README with cleaner structure and code-first approach
- Added Targets section explaining unified `:port`, `PID`, `name` syntax
- Removed "Why proc?" comparison section

### Fixed

- Fixed README filter table incorrectly stating `--in` and `--path` work with `tree` (ps-only)

## [1.0.1] - 2026-01-13

### Fixed

- Fixed outdated error message suggesting removed `proc find --all` command
- Fixed `--cwd` references in help text and documentation (renamed to `--in`)
- Fixed JSON output action field from "find" to "ps"

## [1.0.0] - 2026-01-12

Initial public release.

### Commands

All commands accept **targets**: `:port`, `PID`, or `name` where applicable.

**Discovery** (nouns — observe state)
- `proc on <target>` — Bidirectional port/process lookup
  - `:port` → What process is using this port?
  - `PID` → What ports is this process using?
  - `name` → What ports are these processes using?
- `proc ports` — List all listening ports
- `proc ps [name]` — List processes (filter by name, path, or resources)
- `proc info <target>` — Detailed process information
- `proc tree [target]` — Process hierarchy view
- `proc stuck` — Find hung processes

**Lifecycle** (verbs — change state)
- `proc kill <target>` — Force kill (SIGKILL)
- `proc stop <target>` — Graceful stop (SIGTERM, then SIGKILL after timeout)
- `proc unstick [target]` — Attempt to recover stuck processes
  - Tries SIGCONT → SIGINT recovery sequence
  - Use `--force` to terminate if recovery fails

### Features

- **Unified targets**: Most commands accept `:port`, `PID`, or process `name`
- **Path filtering**: `--in` and `--path` filters for `proc ps`
- **Resource filtering**: `--min-cpu`, `--min-mem`, `--status` filters for `ps` and `tree`
- **Bidirectional lookup**: `proc on` works both ways (port→process and process→ports)
- **Cross-platform**: macOS (Apple Silicon, Intel), Linux (x86_64, ARM64), Windows (x86_64)
- **Output formats**: Colored terminal output, JSON (`--json`) for scripting
- **Safety**: Confirmation prompts before destructive actions

### Principles

- **Semantic**: Commands mean what they say
- **Explicit**: User intent must be clear
- **Complete**: Cover the full workflow, nothing more
- **Fast**: Sub-100ms for all operations
- **Obvious**: If you have to read the docs, we failed

### Values

- **Unified targets**: `:port`, `PID`, and `name` work the same way everywhere
- **Natural grammar**: Nouns to observe, verbs to act
- **Practical simplicity**: Every feature solves a real, repeated problem
- **Easy to remember**: Consistent patterns—know one command, know them all

---

[Unreleased]: https://github.com/yazeed/proc/compare/v1.7.1...HEAD
[1.7.1]: https://github.com/yazeed/proc/compare/v1.7.0...v1.7.1
[1.7.0]: https://github.com/yazeed/proc/compare/v1.6.0...v1.7.0
[1.6.0]: https://github.com/yazeed/proc/compare/v1.5.1...v1.6.0
[1.5.1]: https://github.com/yazeed/proc/compare/v1.5.0...v1.5.1
[1.5.0]: https://github.com/yazeed/proc/compare/v1.4.2...v1.5.0
[1.4.2]: https://github.com/yazeed/proc/compare/v1.4.1...v1.4.2
[1.4.1]: https://github.com/yazeed/proc/compare/v1.4.0...v1.4.1
[1.4.0]: https://github.com/yazeed/proc/compare/v1.3.3...v1.4.0
[1.3.3]: https://github.com/yazeed/proc/compare/v1.3.2...v1.3.3
[1.3.2]: https://github.com/yazeed/proc/compare/v1.3.1...v1.3.2
[1.3.1]: https://github.com/yazeed/proc/compare/v1.3.0...v1.3.1
[1.3.0]: https://github.com/yazeed/proc/compare/v1.2.3...v1.3.0
[1.2.3]: https://github.com/yazeed/proc/compare/v1.2.2...v1.2.3
[1.2.2]: https://github.com/yazeed/proc/compare/v1.2.1...v1.2.2
[1.2.1]: https://github.com/yazeed/proc/compare/v1.2.0...v1.2.1
[1.2.0]: https://github.com/yazeed/proc/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/yazeed/proc/compare/v1.0.2...v1.1.0
[1.0.2]: https://github.com/yazeed/proc/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/yazeed/proc/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/yazeed/proc/releases/tag/v1.0.0
