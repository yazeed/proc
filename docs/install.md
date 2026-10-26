---
layout: page
title: install
description: "install proc on macOS, Linux, or Windows."
permalink: /install
---

## macOS

### homebrew

```bash
brew install yazeed/proc/proc
```

### shell script

```bash
curl -fsSL https://raw.githubusercontent.com/yazeed/proc-cli/main/install.sh | bash
```

## Linux

### shell script

```bash
curl -fsSL https://raw.githubusercontent.com/yazeed/proc-cli/main/install.sh | bash
```

### cargo

```bash
cargo install proc-cli
```

### npm

```bash
npm install -g proc-cli
```

## Windows

### scoop

```bash
scoop bucket add proc https://github.com/yazeed/scoop-bucket-proc
scoop install proc
```

### cargo

```bash
cargo install proc-cli
```

## cross-platform

### cargo

```bash
cargo install proc-cli
```

### cargo-binstall

Pre-built binaries, no compilation.

```bash
cargo binstall proc-cli
```

### npm / bun

```bash
npm install -g proc-cli
# or
bun install -g proc-cli
```

### nix

```bash
nix profile install github:yazeed/proc-cli
```

### docker

Runs with the host PID and network namespaces: `--pid=host` shows the host's processes, and `--net=host` shows its ports (`on :port`, `ports`, `free`).

```bash
docker run --rm -it --pid=host --net=host yazeed/proc-cli
```

## verify

```bash
proc --version
proc on :3000
proc --help
```

## agent skill

Teach your coding agent (Claude Code, Codex, Cursor, Gemini CLI and others) to use proc with structured JSON output.

```bash
npx skills add yazeed/proc-cli
```

Or, in Claude Code, as a plugin:

```text
/plugin marketplace add yazeed/proc-cli
/plugin install proc-cli@proc-cli
```

## shell completions

```bash
# bash
proc completions bash > /etc/bash_completion.d/proc

# zsh
proc completions zsh > ~/.zsh/completions/_proc

# fish
proc completions fish > ~/.config/fish/completions/proc.fish
```

## man page

```bash
proc manpage | sudo tee /usr/local/share/man/man1/proc.1 > /dev/null
man proc
```
