# CLAUDE.md

Project-specific instructions for Claude Code.

## Working Style

**Always ask before committing.** Show the diff or summary and wait for explicit approval before running `git commit`.

**Ask before destructive actions.** This includes `git checkout`, `git reset`, file deletions, or anything that can't be easily undone.

**Show work before publishing.** Before running release workflows, publishing to package managers, or pushing tags, summarize what will happen and wait for approval.

## Project Overview

proc is a semantic CLI tool for process management written in Rust. It provides a unified interface for process and port operations across macOS, Linux, and Windows.

**Tagline (use exactly):** "Semantic CLI tool for process management. Target by port, PID, name or path."

**Repository:** https://github.com/yazeed/proc

## Architecture

```
src/
├── main.rs           # CLI entry point, clap setup, command routing
├── lib.rs            # Library exports
├── error.rs          # Error types and exit codes
├── commands/         # One file per command
│   ├── mod.rs        # Exports all commands
│   ├── kill.rs       # proc kill
│   ├── stop.rs       # proc stop
│   ├── on.rs         # proc on (bidirectional port/process lookup)
│   ├── by.rs         # proc by (filter by name)
│   ├── find_in.rs    # proc in (filter by directory, named find_in to avoid Rust keyword)
│   ├── list.rs       # proc list
│   ├── info.rs       # proc info
│   ├── ports.rs      # proc ports
│   ├── tree.rs       # proc tree
│   ├── stuck.rs      # proc stuck
│   └── unstick.rs    # proc unstick
├── core/             # Shared logic
│   ├── mod.rs        # Exports
│   ├── target.rs     # Target parsing (:port, PID, name) and resolution
│   ├── process.rs    # Process operations
│   └── port.rs       # Port operations
└── ui/               # Output formatting
    ├── mod.rs
    └── output.rs     # Printer, OutputFormat (Human/Json)
```

## Code Style

### Command Pattern

Each command follows this structure:

```rust
//! `proc <cmd>` - Brief description
//!
//! Examples:
//!   proc cmd target          # What it does
//!   proc cmd :3000           # Port example
//!   proc cmd --flag          # Flag example

use crate::core::{...};
use crate::error::{ProcError, Result};
use crate::ui::{OutputFormat, Printer};
use clap::Args;

/// Brief doc for --help
#[derive(Args, Debug)]
pub struct CmdCommand {
    /// Target description
    pub target: String,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Output as JSON
    #[arg(long, short = 'j')]
    pub json: bool,

    /// Show verbose output
    #[arg(long, short = 'v')]
    pub verbose: bool,
}

impl CmdCommand {
    pub fn execute(&self) -> Result<()> {
        // 1. Setup printer
        let format = if self.json { OutputFormat::Json } else { OutputFormat::Human };
        let printer = Printer::new(format, self.verbose);

        // 2. Parse and resolve targets
        let targets = parse_targets(&self.target);
        let (processes, not_found) = resolve_targets(&targets);

        // 3. Warn about not-found targets
        for target in &not_found {
            printer.warning(&format!("Target not found: {}", target));
        }

        // 4. Handle dry-run if applicable
        // 5. Confirm destructive actions (unless --yes)
        // 6. Execute operation
        // 7. Print results

        Ok(())
    }
}
```

### Conventions

- **Unified targets**: `:port`, `PID`, `name` work everywhere
- **Multi-target**: Comma-separated, deduplicated by PID
- **Confirmation**: Destructive commands require `--yes` or interactive confirmation
- **Dry-run**: Destructive commands support `--dry-run`
- **JSON output**: All commands support `--json` for scripting
- **Short flags**: `-y` (yes), `-j` (json), `-v` (verbose)

### Error Handling

Use `ProcError` variants from `src/error.rs`. Return `Result<()>` from execute methods.

## Building & Testing

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo test               # Run tests
cargo fmt                # Format code
cargo clippy             # Lint
```

Pre-commit hook runs `cargo fmt --check` automatically.

## Release Process

Releases are automated via GitHub Actions. On tag push:

1. **Build**: Cross-compile for all platforms
2. **GitHub Release**: Create release with binaries
3. **crates.io**: Publish with `--allow-dirty`
4. **npm**: Publish proc-cli package
5. **Homebrew**: Update yazeed/homebrew-proc tap
6. **Scoop**: Update yazeed/scoop-bucket-proc bucket
7. **Docker**: Push to yazeed/proc on Docker Hub

**Version files to update:**
- `Cargo.toml` (version field)
- `flake.nix` (version field)
- `pkg/npm/package.json` (version field)
- `CHANGELOG.md` (new section)

## Package Distribution

| Channel | Package Name | Install Command |
|---------|-------------|-----------------|
| crates.io | proc-cli | `cargo install proc-cli` |
| npm | proc-cli | `npm install -g proc-cli` |
| Homebrew | proc | `brew install yazeed/proc/proc` |
| Scoop | proc | `scoop install yazeed/scoop-bucket-proc/proc` |
| Docker | yazeed/proc | `docker run yazeed/proc` |
| Nix | — | `nix profile install github:yazeed/proc` |

## Documentation Files

- `README.md` — Main documentation, installation, examples
- `CHANGELOG.md` — Keep a Changelog format, update with each release
- `ROADMAP.md` — Version plans, feature status, under consideration
- `PHILOSOPHY.md` — Project manifesto, principles, feature test
- `CONTRIBUTING.md` — Contribution guidelines
- `SECURITY.md` — Security policy
