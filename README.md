# proc

[![CI](https://github.com/yazeed/proc/workflows/CI/badge.svg)](https://github.com/yazeed/proc/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/proc-cli.svg)](https://crates.io/crates/proc-cli)

**proc** is a semantic command-line tool that makes process management intuitive, cross-platform, and AI-centric.

## Why proc?

Managing processes shouldn't require memorizing complex commands. proc uses semantic commands that do exactly what they say:

### Before (Complex)
```bash
lsof -i :3000 | grep LISTEN | awk '{print $2}' | xargs kill -9
ps aux | grep node | grep -v grep | awk '{print $2}' | xargs kill -9
```

### After (Semantic)
```bash
proc kill :3000          # Kill what's on port 3000
proc kill node           # Kill all Node.js processes
```

## Quick Start

### Installation

**Homebrew (macOS/Linux)**
```bash
brew install yazeed/tap/proc
```

**Cargo (Rust)**
```bash
cargo install proc-cli
```

**Direct Download**
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
```

### Basic Usage

```bash
# Find processes by name
proc find node
proc find python

# What's on a port?
proc on :3000

# List all listening ports
proc ports

# Kill processes
proc kill node           # Kill by name
proc kill :3000          # Kill by port
proc kill 1234           # Kill by PID

# Find stuck processes
proc stuck
proc stuck --timeout 60  # Stuck > 1 minute
```

## Commands

| Command | Alias | Description | Example |
|---------|-------|-------------|---------|
| `find` | `f` | Find processes by name | `proc find node` |
| `on` | `:` | What's on a port? | `proc on :3000` |
| `ports` | `p` | List listening ports | `proc ports` |
| `kill` | `k` | Kill process(es) | `proc kill node` |
| `stuck` | `x` | Find hung processes | `proc stuck` |

## Features

- **Semantic Commands**: Commands mean what they say
- **Cross-Platform**: Works on macOS, Linux, and Windows (WSL)
- **Beautiful Output**: Colored terminal output
- **JSON Support**: Use `--json` for scripting
- **Safe by Default**: Confirmation prompts before destructive actions
- **Fast**: Sub-100ms response time

## Examples

### Find and Kill Node.js Processes
```bash
$ proc find node
✓ Found 3 processes

PID      NAME                      CPU%   MEM (MB)     STATUS
─────────────────────────────────────────────────────────────────
12345    node                       2.3       156.4    Running
12346    node                       0.1        45.2    Running
12347    node                       0.0        12.1    Sleeping

$ proc kill node
⚠ Found 3 processes to kill:

  → node [PID 12345] - CPU: 2.3%, MEM: 156.4MB
  → node [PID 12346] - CPU: 0.1%, MEM: 45.2MB
  → node [PID 12347] - CPU: 0.0%, MEM: 12.1MB

Kill 3 processes? [y/N]: y
✓ Killed 3 processes
```

### Check What's Using a Port
```bash
$ proc on :3000
✓ Process on port 3000:

  Name: node
  PID: 12345
  Protocol: Tcp
  Address: 0.0.0.0
```

### List All Listening Ports
```bash
$ proc ports
✓ Found 5 listening ports

PORT     PROTO      PID      PROCESS              ADDRESS
─────────────────────────────────────────────────────────────────
3000     TCP        12345    node                 0.0.0.0
5432     TCP        789      postgres             127.0.0.1
6379     TCP        456      redis                127.0.0.1
8080     TCP        123      nginx                0.0.0.0
11434    TCP        999      ollama               127.0.0.1
```

### JSON Output for Scripting
```bash
$ proc find node --json
{
  "action": "find",
  "success": true,
  "count": 1,
  "processes": [
    {
      "pid": 12345,
      "name": "node",
      "cpu_percent": 2.3,
      "memory_mb": 156.4,
      "status": "running"
    }
  ]
}
```

## Options

Most commands support these options:

| Option | Short | Description |
|--------|-------|-------------|
| `--json` | `-j` | Output as JSON |
| `--verbose` | `-v` | Show more details |
| `--yes` | `-y` | Skip confirmation prompts |
| `--dry-run` | | Show what would happen without doing it |
| `--help` | `-h` | Show help |

## Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| macOS (Apple Silicon) | ✅ Full | Primary development platform |
| macOS (Intel) | ✅ Full | |
| Linux (x86_64) | ✅ Full | |
| Linux (ARM64) | ✅ Full | |
| Windows (WSL) | ✅ Full | Native Windows coming soon |

## Building from Source

```bash
# Clone the repository
git clone https://github.com/yazeed/proc
cd proc

# Build
cargo build --release

# Run
./target/release/proc --help

# Install locally
cargo install --path .
```

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT License - see [LICENSE](LICENSE) for details.

## Roadmap

### v0.5 (Coming Soon)
- `proc info <pid>` - Detailed process information
- `proc tree` - Process hierarchy view
- `proc stop` - Graceful termination (SIGTERM)
- `proc unstick` - Kill stuck processes + cleanup

### v1.0
- `proc watch` - Real-time process monitoring
- `proc tail` - Follow process output
- `proc hog` - Find resource hogs
- Shell completions (bash, zsh, fish)
- Man pages

## Support

- [GitHub Issues](https://github.com/yazeed/proc/issues) - Bug reports and feature requests
- [Discussions](https://github.com/yazeed/proc/discussions) - Questions and ideas

---

**proc**: The semantic process manager. Built for modern developers.
