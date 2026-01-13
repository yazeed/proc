# proc

[![CI](https://github.com/yazeed/proc/workflows/CI/badge.svg)](https://github.com/yazeed/proc/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/proc-cli.svg)](https://crates.io/crates/proc-cli)

**proc** is a command-line tool for developers who are tired of arcane incantations for simple process tasks.

## Why proc?

Every developer knows this pain:

```bash
# What's on port 3000?
lsof -i :3000 | grep LISTEN | awk '{print $2}'

# Kill it
lsof -i :3000 | grep LISTEN | awk '{print $2}' | xargs kill -9

# Find all node processes
ps aux | grep node | grep -v grep
```

With proc:

```bash
proc on :3000      # What's on port 3000?
proc kill :3000    # Kill it
proc ps node       # List node processes
```

Commands that mean what they say.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/yazeed/proc/main/install.sh | bash
```

Or via package managers:

```bash
brew install yazeed/proc/proc   # Homebrew
cargo install proc-cli          # Cargo
```

<details>
<summary>Manual download</summary>

```bash
# macOS (Apple Silicon)
curl -fsSL https://github.com/yazeed/proc/releases/latest/download/proc-darwin-aarch64 -o proc
chmod +x proc && sudo mv proc /usr/local/bin/

# macOS (Intel)
curl -fsSL https://github.com/yazeed/proc/releases/latest/download/proc-darwin-x86_64 -o proc
chmod +x proc && sudo mv proc /usr/local/bin/

# Linux (x86_64)
curl -fsSL https://github.com/yazeed/proc/releases/latest/download/proc-linux-x86_64 -o proc
chmod +x proc && sudo mv proc /usr/local/bin/

# Linux (ARM64)
curl -fsSL https://github.com/yazeed/proc/releases/latest/download/proc-linux-aarch64 -o proc
chmod +x proc && sudo mv proc /usr/local/bin/

# Windows (PowerShell)
Invoke-WebRequest -Uri https://github.com/yazeed/proc/releases/latest/download/proc-windows-x86_64.exe -OutFile proc.exe
Move-Item proc.exe C:\Windows\System32\proc.exe
```
</details>

## Commands

All commands accept **targets**: `:port`, `PID`, or `name`.

### Discovery

```bash
proc on :3000          # What's using port 3000?
proc on 1234           # What ports is PID 1234 using?
proc on node           # What ports are node processes using?
proc ports             # List all listening ports
proc ps                # List all processes
proc ps node           # Filter by name
proc ps --in .        # Processes in current directory
proc ps --min-cpu 10   # Processes using >10% CPU
proc info :3000        # Info for process on port 3000
proc info 1234         # Info for PID 1234
proc tree              # Full process hierarchy
proc tree :3000        # Tree for process on port 3000
proc tree --min-cpu 5  # Tree filtered by CPU usage
proc stuck             # Find processes that appear hung
```

### Lifecycle

```bash
proc kill :3000        # Kill process on port 3000
proc kill node         # Kill all node processes (SIGKILL)
proc stop :3000        # Stop gracefully (SIGTERM, then SIGKILL)
proc stop node         # Stop all node processes gracefully
proc unstick           # Attempt to recover stuck processes
proc unstick --force   # Terminate if recovery fails
```

## Command Reference

| Command | Alias | Description |
|---------|-------|-------------|
| `on` | `:` | Port/process lookup (bidirectional) |
| `ports` | `p` | List listening ports |
| `ps` | `l` | List processes |
| `info` | `i` | Detailed process info |
| `tree` | `t` | Process hierarchy |
| `kill` | `k` | Force kill (SIGKILL) |
| `stop` | `s` | Graceful stop (SIGTERM) |
| `stuck` | `x` | Find hung processes |
| `unstick` | `u` | Attempt to recover stuck processes |

## Common Options

| Option | Short | Description |
|--------|-------|-------------|
| `--json` | `-j` | Output as JSON for scripting |
| `--verbose` | `-v` | Show paths, cwd, and full commands |
| `--yes` | `-y` | Skip confirmation prompts |
| `--dry-run` | | Preview actions without executing |
| `--force` | `-f` | Force action (e.g., terminate if recovery fails) |

## Filter Options

Available on `ps` and `tree`:

| Option | Description |
|--------|-------------|
| `--in <path>` | Filter by working directory |
| `--path <path>` | Filter by executable path |
| `--min-cpu <n>` | Only processes using >n% CPU |
| `--min-mem <n>` | Only processes using >n MB memory |
| `--status <s>` | Filter by status: running, sleeping, stopped, zombie |

## Examples

### Port lookup

```bash
$ proc on :3000
✓ Port 3000 is used by:
  Process: node (PID 12345)
  Path: /usr/local/bin/node
  Listening: TCP on 0.0.0.0
  Resources: 2.3% CPU, 156.4 MB
  Uptime: 2h 34m

$ proc on 12345
✓ node (PID 12345) is listening on:
  → :3000 (TCP on 0.0.0.0)
  → :3001 (TCP on 127.0.0.1)
```

### Process discovery

```bash
$ proc ps --in /my/project
✓ Found 3 processes

PID      NAME        CPU%   MEM (MB)   STATUS
──────────────────────────────────────────────
12345    node        2.3    156.4      Running
12346    npm         0.1    45.2       Sleeping

$ proc tree --min-cpu 5
✓ 2 processes matching filters:
├── ● node [12345] 12.3% 256.4MB
└── ● python [12400] 8.1% 128.2MB
```

### Lifecycle management

```bash
$ proc kill :3000
Kill node [PID 12345]? [y/N]: y
✓ Killed 1 process

$ proc unstick
! Found 1 stuck process:
  → node [PID 12345] - 98.2% CPU, running for 2h 15m
Unstick 1 process? [y/N]: y
  → node [PID 12345]... recovered
✓ 1 process recovered
```

### Scripting with JSON

```bash
$ proc ps node --json | jq '.processes[].pid'
12345
12346
```

## Platform Support

| Platform | Status |
|----------|--------|
| macOS (Apple Silicon) | ✅ |
| macOS (Intel) | ✅ |
| Linux (x86_64) | ✅ |
| Linux (ARM64) | ✅ |
| Windows (x86_64) | ✅ |

## Building from Source

```bash
git clone https://github.com/yazeed/proc
cd proc
cargo build --release
./target/release/proc --help
```

## Roadmap

See [ROADMAP.md](ROADMAP.md) for planned features.

## Contributing

Contributions welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT - see [LICENSE](LICENSE).

---

**proc**: Process management for humans.
