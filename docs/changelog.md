---
layout: page
title: changelog
description: "release history for proc."
permalink: /changelog
---

## 1.8.1 — 2026-03-06

**Fixed**
- README demo image: use absolute URL so it renders on Docker Hub, crates.io, and npm

**Added**
- GitHub Pages blog with 7 technical posts
- Install page with all platforms, shell completions, and man page setup
- Blog redesign: terminal editorial theme with syntax highlighting and SEO meta tags

## 1.8.0 — 2026-03-06

**Added**
- `proc watch`: real-time process monitoring with auto-refresh
  - aliases: `proc top`, `proc w`
  - configurable interval (`-n`), sort (`-s`), limit (`-l`)
  - combines with `--in`, `--by`, `--min-cpu`, `--min-mem`
  - alternate screen with clean terminal restore on exit
  - NDJSON output (`--json`) for streaming
  - non-TTY detection: single snapshot when piped

**Fixed**
- multi-target "not found" output consolidated into single warning

## 1.7.4 — 2026-03-01

**Fixed**
- self-exclusion: proc no longer shows itself in results

## 1.7.3 — 2026-02-26

**Added**
- working directory in output: see which project folder a process runs from

**Changed**
- table `PATH` column replaced with `DIR` (working directory)

---

[full changelog on github](https://github.com/yazeed/proc/blob/main/CHANGELOG.md)
