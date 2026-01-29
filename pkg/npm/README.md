# proc-cli

Semantic CLI tool for process management. Target by port, process id (PID), name or path.

## Install

```bash
npm install -g proc-cli
yarn global add proc-cli
pnpm add -g proc-cli
bun install -g proc-cli
```

## Usage

```bash
proc on :3000           # what's on port 3000?
proc kill :3000,:8080   # kill multiple targets
proc by node --in .     # node processes in current directory
```

## Documentation

**[Full documentation on GitHub](https://github.com/yazeed/proc)**

## About

This package downloads the native `proc` binary for your platform. The binary is written in Rust for maximum performance.

## License

MIT
