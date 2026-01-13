# proc

[![CI](https://github.com/yazeed/proc/workflows/CI/badge.svg)](https://github.com/yazeed/proc/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/proc-cli.svg)](https://crates.io/crates/proc-cli)

A semantic process manager. Commands that read like sentences.

```bash
proc on :3000       # what's on port 3000?
proc kill :3000     # kill it
proc ps node        # list node processes
proc stop node      # stop them gracefully
```

## Install

```bash
brew install yazeed/proc/proc   # Homebrew
cargo install proc-cli          # Cargo
```

<details>
<summary>Other methods</summary>

**Script:**
```bash
curl -fsSL https://raw.githubusercontent.com/yazeed/proc/main/install.sh | bash
```

**Manual:**
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

## Usage

### Targets

Every command accepts the same target syntax:

| Target | Example | Meaning |
|--------|---------|---------|
| `:port` | `:3000` | Process using port 3000 |
| `PID` | `12345` | Process with ID 12345 |
| `name` | `node` | All processes named "node" |

### Discovery

```bash
proc on :3000          # what's using port 3000?
proc on 12345          # what ports is PID 12345 using?
proc on node           # what ports are node processes using?

proc ports             # all listening ports
proc ps                # all processes
proc ps node           # filter by name
proc ps --in .         # processes started in current directory
proc ps --path /usr    # processes from /usr/*
proc ps --min-cpu 10   # processes using >10% CPU

proc info :3000        # detailed info for process on port 3000
proc tree              # process hierarchy
proc tree --min-cpu 5  # tree filtered by CPU

proc stuck             # find hung processes
```

### Lifecycle

```bash
proc kill :3000        # SIGKILL process on port 3000
proc kill node         # SIGKILL all node processes
proc stop :3000        # SIGTERM, then SIGKILL after timeout
proc stop node         # graceful stop for all node processes

proc unstick           # attempt to recover stuck processes
proc unstick --force   # terminate if recovery fails
```

## Reference

### Commands

| Command | Alias | Description |
|---------|-------|-------------|
| `on` | `:` | Bidirectional port/process lookup |
| `ports` | `p` | List listening ports |
| `ps` | `l` | List processes |
| `info` | `i` | Detailed process info |
| `tree` | `t` | Process hierarchy |
| `kill` | `k` | Force kill (SIGKILL) |
| `stop` | `s` | Graceful stop (SIGTERM) |
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

Available on `ps` and `tree`:

| Option | Description |
|--------|-------------|
| `--in <path>` | Filter by working directory |
| `--path <path>` | Filter by executable path |
| `--min-cpu <n>` | Processes using >n% CPU |
| `--min-mem <n>` | Processes using >n MB memory |
| `--status <s>` | running, sleeping, stopped, zombie |

## Examples

```bash
$ proc on :3000
✓ Port 3000 is used by:
  Process: node (PID 12345)
  Path: /usr/local/bin/node
  Listening: TCP on 0.0.0.0
  Resources: 2.3% CPU, 156.4 MB
  Uptime: 2h 34m

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

$ proc kill :3000
Kill node [PID 12345]? [y/N]: y
✓ Killed 1 process

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
