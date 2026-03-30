# proc

<p align="center">
  <img src="https://raw.githubusercontent.com/yazeed/proc/main/demo.gif" alt="proc demo" width="800" />
</p>

[![CI](https://github.com/yazeed/proc/workflows/CI/badge.svg)](https://github.com/yazeed/proc/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/proc-cli.svg)](https://crates.io/crates/proc-cli)
[![Docker](https://img.shields.io/docker/v/yazeed/proc?label=docker)](https://hub.docker.com/r/yazeed/proc)
[![npm](https://img.shields.io/npm/v/proc-cli)](https://www.npmjs.com/package/proc-cli)
[![Changelog](https://img.shields.io/crates/v/proc-cli?label=changelog&color=blue)](CHANGELOG.md)
[![Downloads](https://img.shields.io/crates/d/proc-cli.svg)](https://crates.io/crates/proc-cli)
[![Open Collective](https://img.shields.io/opencollective/all/proc-cli?label=backers)](https://opencollective.com/proc-cli)
[![SafeSkill 92/100](https://img.shields.io/badge/SafeSkill-92%2F100-green)](https://safeskill.dev/scan/proc-cli)
[![Website](https://img.shields.io/website?url=https%3A%2F%2Fyazeed.github.io%2Fproc%2F&label=website)](https://yazeed.github.io/proc/)

Semantic CLI tool for process management. Target by port, process id (PID), name or path.

```bash
proc on :3000                   # what's on port 3000?
proc for ./script.py            # what's running this file?
proc by node --in . --min-cpu 5 # node in cwd using >5% CPU
proc kill :3000,:8080,node      # kill mixed targets at once
proc info :3000,1234            # info for port + PID
```

### Why proc?

| Task | proc | Without proc |
|------|------|-------------|
| What's on port 3000? | `proc on :3000` | `lsof -i :3000 -P -n` |
| Kill it | `proc kill :3000` | `lsof -i :3000 -t \| xargs kill -9` |
| Free port 3000 | `proc free :3000` | `lsof -i :3000 -t \| xargs kill && sleep 1 && lsof -i :3000` |
| Why is port busy? | `proc why :3000` | `lsof -i :3000` + `ps -o ppid=` chain |
| Pause a process | `proc freeze node` | `kill -STOP $(pgrep node)` |
| Find orphans | `proc orphans --in .` | No standard tool |
| Wait for process | `proc wait node` | `while ps aux \| grep node \| grep -v grep; do sleep 5; done` |
| Ports used by node | `proc on node` | `lsof -i -P -n \| grep node` |
| Node processes in cwd | `proc by node --in .` | `ps aux \| grep node` + manual check |
| Kill mixed targets | `proc kill :3000,:8080,node` | Three separate commands |
| Watch in real-time | `proc watch node` | `watch -n2 'ps aux \| grep node'` |
| What runs this file? | `proc for ./app.js` | `lsof ./app.js` or `fuser ./app.js` |

One syntax. One tool. [Full comparison &rarr;](https://yazeed.github.io/proc/posts/proc-vs-lsof-fuser-killport-fkill/)

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

**What's running this script?**
```bash
proc for ./script.py
```

**Find all node processes in current directory using >5% CPU:**
```bash
proc by node --in . --min-cpu 5
```

**Kill multiple targets at once:**
```bash
proc kill :3000,:8080,node
```

**Watch processes in real-time:**
```bash
proc watch node --in .
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
| `for <file>` | `f` | Find processes by file path |
| `by <name>` | `b` | Filter processes by name |
| `in <path>` | | Filter processes by working directory |
| `list` | `l`, `ps` | List all processes |
| `info <target>` | `i` | Detailed process information |
| `ports` | `p` | List all listening ports |
| `tree` | `t` | Process hierarchy |
| `watch` | `w`, `top` | Real-time process monitoring |
| `why <target>` | | Trace process ancestry (why is this port busy?) |
| `orphans` | `o` | Find orphaned processes |
| `stuck` | `x` | Find hung processes |

### Lifecycle

| Command | Alias | Description |
|---------|-------|-------------|
| `kill <target>` | `k` | Force kill (SIGKILL) |
| `stop <target>` | `s` | Graceful stop (SIGTERM, `--signal` for custom) |
| `freeze <target>` | | Pause process (SIGSTOP) |
| `thaw <target>` | | Resume frozen process (SIGCONT) |
| `free <:port>` | | Kill process and verify port freed |
| `wait <target>` | | Block until process(es) exit |
| `unstick` | `u` | Recover stuck processes |

### Filters

Filters can be combined with discovery and lifecycle commands:

| Filter | Description |
|--------|-------------|
| `--in <path>` | Filter by working directory |
| `--by <name>` | Filter by process name |
| `--path <path>` | Filter by executable path (`list`) |
| `--min-cpu <n>` | Processes using >n% CPU |
| `--min-mem <n>` | Processes using >n MB memory |
| `--min-uptime <s>` | Processes running longer than s seconds |
| `--parent <pid>` | Children of a specific parent PID |
| `--status <s>` | Filter by status: running, sleeping, stopped, zombie |
| `--exposed` | Only network-exposed ports (`ports`) |
| `--local` | Only localhost ports (`ports`) |
| `--range <s-e>` | Filter ports by range (e.g., `3000-9000`) |
| `--sort/-s <key>` | Sort by: cpu, mem, pid, name |
| `--limit/-n <n>` | Limit number of results |
| `--ancestors/-a` | Show ancestry path to root (`tree`) |
| `--depth/-d <n>` | Maximum tree depth (default: 10) |
| `--compact/-C` | Show PIDs only in compact view (`tree`) |
| `--timeout/-t <s>` | Seconds before considered stuck (default: 300) |

### Options

| Option | Short | Description |
|--------|-------|-------------|
| `--json` | `-j` | JSON output |
| `--verbose` | `-v` | Show paths, cwd, full commands |
| `--yes` | `-y` | Skip confirmation |
| `--dry-run` | | Preview without executing |
| `--force` | `-f` | Force termination if recovery fails (`unstick`) |
| `--graceful` | `-g` | Send SIGTERM instead of SIGKILL (`kill`) |
| `--signal` | `-S` | Custom signal: HUP, INT, USR1, etc. (`stop`) |
| `--kill` | `-k` | Kill found processes (`stuck`, `orphans`) |
| `--wait` | | Max seconds to wait for port to free (`free`) |
| `--interval` | `-n` | Poll/refresh interval in seconds (`wait`, `watch`) |
| `--timeout` | `-t` | Max seconds to wait before giving up (`wait`, `stop`) |
| `--quiet` | `-q` | Suppress periodic status messages (`wait`) |

## Examples

```bash
# What's on port 3000?
proc on :3000

# What ports is node using?
proc on node

# What's running this file?
proc for ./script.py

# What has this file open?
proc for /var/log/app.log

# Node processes in current directory
proc by node --in .

# Processes using >10% CPU
proc list --min-cpu 10

# Kill everything on ports 3000 and 8080
proc kill :3000,:8080

# Kill only node processes in current directory
proc kill node --in . --dry-run

# Process tree filtered by CPU usage
proc tree --min-cpu 5

# Watch all processes sorted by CPU (alias: proc top)
proc watch

# Watch node processes with 1s refresh
proc watch node -n 1

# Watch process on port 3000
proc watch :3000

# Watch processes in cwd sorted by memory
proc watch --in . --sort mem

# Freeze/thaw (pause and resume)
proc freeze node               # Pause all node processes
proc thaw node                 # Resume them

# Free a port (kill + verify available)
proc free :3000                # Kill and confirm port 3000 is free
proc free :3000,:8080 --yes    # Free multiple ports

# Why is port 3000 busy?
proc why :3000                 # Show ancestry chain with port context

# Find orphaned processes
proc orphans                   # List orphans
proc orphans --in . --kill     # Kill orphans in current directory

# Wait for processes to exit
proc wait node                 # Block until all node processes exit
proc wait :3000 --timeout 60   # Wait up to 60s for port process to exit
proc wait node -n 10 -q        # Check every 10s, quiet mode

# Send custom signal
proc stop nginx --signal HUP   # Reload config (SIGHUP)

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

## Shell Completions

Generate completions for your shell:

```bash
# Bash
proc completions bash > /etc/bash_completion.d/proc

# Zsh
proc completions zsh > ~/.zsh/completions/_proc

# Fish
proc completions fish > ~/.config/fish/completions/proc.fish
```

## Man Page

Generate and install the man page:

```bash
proc manpage | sudo tee /usr/local/share/man/man1/proc.1 > /dev/null
```

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
