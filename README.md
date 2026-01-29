# proc

<img width="894" height="400" alt="proc-logo" src="https://github.com/user-attachments/assets/b4b44b5c-d94d-4cc2-9fda-d572c3544131" /><br/>

[![CI](https://github.com/yazeed/proc/workflows/CI/badge.svg)](https://github.com/yazeed/proc/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/proc-cli.svg)](https://crates.io/crates/proc-cli)
[![Docker](https://img.shields.io/docker/v/yazeed/proc?label=docker)](https://hub.docker.com/r/yazeed/proc)
[![npm](https://img.shields.io/npm/v/proc-cli)](https://www.npmjs.com/package/proc-cli)
[![Changelog](https://img.shields.io/crates/v/proc-cli?label=changelog&color=blue)](CHANGELOG.md)
[![Downloads](https://img.shields.io/crates/d/proc-cli.svg)](https://crates.io/crates/proc-cli)
[![Open Collective](https://img.shields.io/opencollective/all/proc-cli?label=backers)](https://opencollective.com/proc-cli)

Semantic CLI tool for process management. Target by port, process id (PID), name or path.

```bash
proc on :3000                   # what's on port 3000?
proc on :3000,:8080,node        # multi-target: ports + name
proc by node --in . --min-cpu 5 # node in cwd using >5% CPU
proc kill :3000,:8080,node -y   # kill mixed targets at once
proc info :3000,1234            # info for port + PID
```

## Install

| Platform | Command |
|----------|---------|
| macOS | `brew install yazeed/proc/proc` |
| Windows | `scoop bucket add proc https://github.com/yazeed/scoop-bucket-proc && scoop install proc` |
| Rust | `cargo install proc-cli` |
| npm/bun | `npm install -g proc-cli` |
| Nix | `nix profile install github:yazeed/proc` |
| Docker | `docker run --rm -it --pid=host yazeed/proc` |
| Shell | `curl -fsSL https://raw.githubusercontent.com/yazeed/proc/main/install.sh \| bash` |

<details>
<summary>More options</summary>

| Platform | Method | Command |
|----------|--------|---------|
| macOS/Linux | cargo-binstall | `cargo binstall proc-cli` |
| Arch Linux | AUR | `yay -S proc` (pending) |

**Manual download:**

```bash
# macOS (Apple Silicon)
curl -fsSL https://github.com/yazeed/proc/releases/latest/download/proc-darwin-aarch64.tar.gz | tar xz
sudo mv proc-darwin-aarch64 /usr/local/bin/proc

# macOS (Intel)
curl -fsSL https://github.com/yazeed/proc/releases/latest/download/proc-darwin-x86_64.tar.gz | tar xz
sudo mv proc-darwin-x86_64 /usr/local/bin/proc

# Linux (x86_64)
curl -fsSL https://github.com/yazeed/proc/releases/latest/download/proc-linux-x86_64.tar.gz | tar xz
sudo mv proc-linux-x86_64 /usr/local/bin/proc

# Linux (ARM64)
curl -fsSL https://github.com/yazeed/proc/releases/latest/download/proc-linux-aarch64.tar.gz | tar xz
sudo mv proc-linux-aarch64 /usr/local/bin/proc

# Windows (PowerShell)
Invoke-WebRequest -Uri https://github.com/yazeed/proc/releases/latest/download/proc-windows-x86_64.exe.zip -OutFile proc.zip
Expand-Archive proc.zip -DestinationPath .
Move-Item proc-windows-x86_64.exe C:\Windows\System32\proc.exe
```

</details>

## Quick Start

**What's using port 3000?**
```bash
proc on :3000
```

**Kill it:**
```bash
proc kill :3000
```

**Find all node processes in current directory using >5% CPU:**
```bash
proc by node --in . --min-cpu 5
```

**Kill multiple targets at once:**
```bash
proc kill :3000,:8080,node -y
```

## Target Syntax

All commands accept the same target syntax:

| Target | Example | Description |
|--------|---------|-------------|
| Port | `:3000` | Process listening on port 3000 |
| PID | `1234` | Process with ID 1234 |
| Name | `node` | All processes named "node" |
| Multi | `:3000,:8080,node` | Comma-separated targets |

## Commands

### Discovery

| Command | Alias | Description |
|---------|-------|-------------|
| `on <target>` | `:` | Bidirectional port/process lookup |
| `by <name>` | `b` | Filter processes by name |
| `in <path>` | | Filter processes by working directory |
| `list` | `l`, `ps` | List all processes |
| `info <target>` | `i` | Detailed process information |
| `ports` | `p` | List all listening ports |
| `tree` | `t` | Process hierarchy |

### Lifecycle

| Command | Alias | Description |
|---------|-------|-------------|
| `kill <target>` | `k` | Force kill (SIGKILL) |
| `stop <target>` | `s` | Graceful stop (SIGTERM) |
| `stuck` | `x` | Find hung processes |
| `unstick` | `u` | Recover stuck processes |

### Filters

Filters can be combined with discovery commands:

| Filter | Description |
|--------|-------------|
| `--in <path>` | Filter by working directory |
| `--by <name>` | Filter by process name |
| `--path <path>` | Filter by executable path |
| `--min-cpu <n>` | Processes using >n% CPU |
| `--min-mem <n>` | Processes using >n MB memory |
| `--status <s>` | Filter by status: running, sleeping, stopped, zombie |

### Options

| Option | Short | Description |
|--------|-------|-------------|
| `--json` | `-j` | JSON output |
| `--verbose` | `-v` | Show paths, cwd, full commands |
| `--yes` | `-y` | Skip confirmation |
| `--dry-run` | | Preview without executing |
| `--force` | `-f` | Force action |

## Examples

```bash
# What's on port 3000?
proc on :3000

# What ports is node using?
proc on node

# Node processes in current directory
proc by node --in .

# Processes using >10% CPU
proc list --min-cpu 10

# Kill everything on ports 3000 and 8080
proc kill :3000,:8080 -y

# Process tree filtered by CPU usage
proc tree --min-cpu 5

# Find and recover stuck processes
proc stuck
proc unstick --force
```

## Platform Support

| Platform | Architecture | Status |
|----------|--------------|--------|
| macOS | Apple Silicon (ARM64) | ✅ |
| macOS | Intel (x86_64) | ✅ |
| Linux | x86_64 | ✅ |
| Linux | ARM64 | ✅ |
| Windows | x86_64 | ✅ |
| Docker | linux/amd64, linux/arm64 | ✅ |

## Building from Source

```bash
git clone https://github.com/yazeed/proc
cd proc
cargo build --release
```

The binary will be at `target/release/proc`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT
