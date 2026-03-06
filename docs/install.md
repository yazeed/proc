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
curl -fsSL https://raw.githubusercontent.com/yazeed/proc/main/install.sh | bash
```

## Linux

### shell script

```bash
curl -fsSL https://raw.githubusercontent.com/yazeed/proc/main/install.sh | bash
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
nix profile install github:yazeed/proc
```

### docker

Runs with host PID namespace for full process visibility.

```bash
docker run --rm -it --pid=host yazeed/proc
```

## verify

```bash
proc --version
proc on :3000
proc --help
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
