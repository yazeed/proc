# proc-cli

Semantic CLI tool for process management. Target by port, PID, name, or path.

```bash
proc on :3000           # what's on port 3000?
proc kill :3000,:8080   # kill multiple targets
proc by node --in .     # node processes in current directory
```

## Install

```bash
npm install -g proc-cli
# or
bun install -g proc-cli
```

## Usage

```bash
# Discovery
proc on :3000              # what's using port 3000?
proc by node               # processes named 'node'
proc in .                  # processes in current directory

# Lifecycle
proc kill :3000            # kill process on port
proc stop node             # stop gracefully

# Multi-target
proc kill :3000,:8080,node # kill multiple targets at once
```

## Documentation

Full documentation: https://github.com/yazeed/proc

## About

This npm package is a wrapper that downloads the native `proc` binary for your platform. The actual binary is written in Rust for maximum performance.

## License

MIT
