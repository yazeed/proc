# proc

<img width="894" height="400" alt="proc-logo" src="https://github.com/user-attachments/assets/b4b44b5c-d94d-4cc2-9fda-d572c3544131" /><br/>

[![CI](https://github.com/yazeed/proc/workflows/CI/badge.svg)](https://github.com/yazeed/proc/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/proc-cli.svg)](https://crates.io/crates/proc-cli)
[![Docker](https://img.shields.io/docker/v/yazeed/proc?label=docker)](https://hub.docker.com/r/yazeed/proc)
[![Changelog](https://img.shields.io/crates/v/proc-cli?label=changelog&color=blue)](CHANGELOG.md)
[![Downloads](https://img.shields.io/crates/d/proc-cli.svg)](https://crates.io/crates/proc-cli)
[![Open Collective](https://img.shields.io/opencollective/all/proc-cli?label=backers)](https://opencollective.com/proc-cli)

Semantic CLI tool for process management. Target by port, process id (PID), name or path.

```bash
proc on :3000           # what's on port 3000?
proc kill :3000,:8080   # kill multiple targets
proc by node --in .     # node processes in current directory
proc in . --by python   # python processes in cwd
```

## Install

### Package Managers

| Platform | Method | Command |
|----------|--------|---------|
| macOS | Homebrew | `brew install yazeed/proc/proc` |
| macOS/Linux | cargo | `cargo install proc-cli` |
| macOS/Linux | cargo-binstall | `cargo binstall proc-cli` |
| Windows | Scoop | `scoop install proc` ¹ |
| Arch Linux | AUR | `yay -S proc` ² |
| NixOS | Nix Flakes | `nix profile install github:yazeed/proc` |
| Any | npm | `npm install -g proc-cli` ³ |
| Any | Docker | `docker run --rm -it --pid=host yazeed/proc` |

<sub>¹ Requires bucket: `scoop bucket add yazeed https://github.com/yazeed/scoop-bucket`</sub><br/>
<sub>² Package pending AUR submission</sub><br/>
<sub>³ Package pending npm publish</sub>

### Shell Script

```bash
curl -fsSL https://raw.githubusercontent.com/yazeed/proc/main/install.sh | bash
```

<details>
<summary>Manual download</summary>

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

## Usage

### Targets

Commands accept the same target syntax, with multi-target support:

| Target | Example | Meaning |
|--------|---------|---------|
| `:port` | `:3000` | Process using port 3000 |
| `PID` | `12345` | Process with ID 12345 |
| `name` | `node` | All processes named "node" |
| Multi | `:3000,:8080,node` | Comma-separated targets |

### Discovery

```bash
# Port/process lookup (multi-target)
proc on :3000              # what's using port 3000?
proc on :3000,:8080        # what's on multiple ports?
proc on node               # what ports are node processes using?

# Filter by name
proc by node               # processes named 'node'
proc by node --in .        # node processes in current directory
proc by node --min-cpu 5   # node processes using >5% CPU

# Filter by directory
proc in .                  # processes in current directory
proc in ~/projects         # processes in ~/projects
proc in . --by node        # node processes in cwd

# List all
proc list                  # all processes
proc list --min-cpu 10     # processes using >10% CPU

# Info (multi-target)
proc info :3000            # detailed info for process on port
proc info :3000,:8080,node # info for multiple targets

proc ports                 # all listening ports
proc tree --min-cpu 5      # process tree filtered by CPU
proc stuck                 # find hung processes
```

### Lifecycle

```bash
# Kill (multi-target)
proc kill :3000            # SIGKILL process on port 3000
proc kill :3000,:8080,node # kill multiple targets at once
proc kill :3000 -y         # skip confirmation

# Stop (multi-target, graceful)
proc stop :3000            # SIGTERM, then SIGKILL after timeout
proc stop :3000,:8080      # stop multiple targets gracefully
proc stop node -y          # skip confirmation

proc unstick               # attempt to recover stuck processes
proc unstick --force       # terminate if recovery fails
```

## Reference

### Commands

| Command | Alias | Description |
|---------|-------|-------------|
| `on` | `:` | Bidirectional port/process lookup |
| `by` | `b` | Filter processes by name |
| `in` | | Filter processes by directory |
| `list` | `l`, `ps` | List all processes |
| `info` | `i` | Detailed process info |
| `ports` | `p` | List listening ports |
| `kill` | `k` | Force kill (SIGKILL) |
| `stop` | `s` | Graceful stop (SIGTERM) |
| `tree` | `t` | Process hierarchy |
| `stuck` | `x` | Find hung processes |
| `unstick` | `u` | Recover stuck processes |

### Options

| Option | Short | Description |
|--------|-------|-------------|
| `--json` | `-j` | JSON output |
| `--verbose` | `-v` | Show paths, cwd, full commands |
| `--yes` | `-y` | Skip confirmation |
| `--dry-run` | | Preview without executing |
| `--force` | `-f` | Force action |

### Filters

| Option | `by` | `in` | `on` | `list` | `tree` | Description |
|--------|:----:|:----:|:----:|:------:|:------:|-------------|
| `--in <path>` | ✓ | | ✓ | ✓ | | Filter by working directory |
| `--by <name>` | | ✓ | | | | Filter by process name |
| `--path <path>` | ✓ | ✓ | | ✓ | | Filter by executable path |
| `--min-cpu <n>` | ✓ | ✓ | | ✓ | ✓ | Processes using >n% CPU |
| `--min-mem <n>` | ✓ | ✓ | | ✓ | ✓ | Processes using >n MB memory |
| `--status <s>` | ✓ | ✓ | | ✓ | ✓ | running, sleeping, stopped, zombie |

## Examples

```bash
$ proc on :3000
✓ Port 3000 is used by:
  Process: node (PID 12345)
  Path: /usr/local/bin/node
  Listening: TCP on 0.0.0.0
  Resources: 2.3% CPU, 156.4 MB
  Uptime: 2h 34m

$ proc list --in /my/project
✓ Found 3 processes

PID      NAME        CPU%   MEM (MB)   STATUS
──────────────────────────────────────────────
12345    node        2.3    156.4      Running
12346    npm         0.1    45.2       Sleeping

$ proc tree --min-cpu 5
✓ 2 processes matching filters:
├── ● node [12345] 12.3% 256.4MB
└── ● python [12400] 8.1% 128.2MB

$ proc kill :3000
Kill node [PID 12345]? [y/N]: y
✓ Killed 1 process

$ proc list node --json | jq '.processes[].pid'
12345
12346
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

## Building

```bash
git clone https://github.com/yazeed/proc
cd proc
cargo build --release
./target/release/proc --help
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT
