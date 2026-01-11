# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial release preparation

## [0.1.0] - 2026-01-08

### Added
- **proc find** - Find processes by name with pattern matching
  - Case-insensitive search
  - Matches process name and command line
  - Sorting by CPU, memory, PID, or name
  - Limit results with `--limit`

- **proc on** - Show what process is using a port
  - Accepts port formats: `:3000` or `3000`
  - Shows process name, PID, and address

- **proc ports** - List all listening ports
  - Filter by process name with `--filter`
  - Sort by port, PID, or name

- **proc kill** - Kill processes by name, PID, or port
  - Smart target detection (name vs PID vs port)
  - Confirmation prompts for safety
  - `--dry-run` mode to preview actions
  - `--yes` flag to skip confirmation
  - `--graceful` for SIGTERM instead of SIGKILL

- **proc stuck** - Find potentially stuck/hung processes
  - Configurable timeout threshold
  - Option to kill found processes with `--kill`

- **Cross-platform support**
  - macOS (Apple Silicon and Intel)
  - Linux (x86_64 and ARM64)
  - Windows via WSL

- **Output formats**
  - Beautiful colored terminal output
  - JSON output for scripting (`--json`)
  - Verbose mode (`--verbose`)

- **Safety features**
  - Confirmation prompts before destructive actions
  - Dry-run mode
  - Clear error messages with suggestions

### Technical
- Built with Rust for performance and reliability
- Uses `sysinfo` crate for cross-platform process info
- Uses `clap` for CLI argument parsing
- Single binary with no runtime dependencies

---

## Version History

- **0.1.0** - Initial MVP release with 5 core commands

[Unreleased]: https://github.com/yazeed/proc/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/yazeed/proc/releases/tag/v0.1.0
