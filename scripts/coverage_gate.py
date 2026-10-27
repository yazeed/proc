#!/usr/bin/env python3
"""Line-coverage gate for proc.

Usage: coverage_gate.py <lcov>... [--exclusions FILE] [--enforce] [--repo-root DIR]

Each lcov `SF:` path is mapped to a repo file by comparing it with the repo's
real src/**/*.rs files (backslashes normalised to '/'; the longest repo path
that the SF path ends with wins). Paths that match no repo file, and anything
under a cargo registry/git checkout (any CARGO_HOME) or the rustc sources, are
ignored.
Line hits are merged across all lcov files (covered if hit in any). Lines in
the exclusions file (`src/path.rs:START-END  reason`, reason required, `#` comments) are dropped;
an exclusion that hides no uncovered line is reported as stale (a failure under
--enforce).
Prints per-file covered/total and every uncovered `src/path.rs:LINE`; with
--enforce, exits 1 if any line is uncovered.
"""

import argparse
import re
import sys
from pathlib import Path

# Cargo registry and git checkouts wherever CARGO_HOME points (it need not be
# called .cargo), and the rustc/std sources. coverage-local.sh's llvm-cov
# -ignore-filename-regex must agree with these.
DEPENDENCY_MARKERS = ("/registry/src/", "/git/checkouts/", "/rustc/")


def repo_sources(root):
    return sorted(
        (p.relative_to(root).as_posix() for p in (root / "src").rglob("*.rs")),
        key=len,
        reverse=True,
    )


def map_path(sf, sources):
    norm = sf.replace("\\", "/")
    if any(m in norm for m in DEPENDENCY_MARKERS):
        return None
    for rel in sources:
        if norm == rel or norm.endswith("/" + rel):
            return rel
    return None


def parse_lcov(path, sources, hits):
    current = None
    with open(path, encoding="utf-8", errors="replace") as f:
        for raw in f:
            line = raw.strip()
            if line.startswith("SF:"):
                current = map_path(line[3:], sources)
            elif line == "end_of_record":
                current = None
            elif line.startswith("DA:") and current is not None:
                fields = line[3:].split(",")
                lineno, count = int(fields[0]), int(fields[1])
                per_file = hits.setdefault(current, {})
                per_file[lineno] = per_file.get(lineno, 0) + count


def parse_exclusions(path, sources):
    """Returns {file: {line: entry_number}} plus the entry list for staleness checks."""
    excluded = {}
    entries = []
    pattern = re.compile(r"^(\S+?):(\d+)(?:-(\d+))?\s+(\S.*)$")
    for n, raw in enumerate(Path(path).read_text().splitlines(), 1):
        text = raw.strip()
        if not text or text.startswith("#"):
            continue
        m = pattern.match(text)
        if not m:
            sys.exit(
                f"{path}:{n}: expected 'src/path.rs:START-END  reason' "
                f"(a reason is required), got: {text}"
            )
        rel = m.group(1)
        start = int(m.group(2))
        end = int(m.group(3) or start)
        if rel not in sources:
            sys.exit(f"{path}:{n}: {rel} is not a source file of this repo")
        if start > end:
            sys.exit(f"{path}:{n}: range {start}-{end} has START after END")
        excluded.setdefault(rel, set()).update(range(start, end + 1))
        entries.append((n, rel, start, end))
    return excluded, entries


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("lcov", nargs="+")
    ap.add_argument("--exclusions")
    ap.add_argument("--enforce", action="store_true")
    ap.add_argument("--repo-root", default=str(Path(__file__).resolve().parent.parent))
    args = ap.parse_args()

    sources = repo_sources(Path(args.repo_root))
    hits = {}
    for path in args.lcov:
        parse_lcov(path, sources, hits)
    excluded, entries = (
        parse_exclusions(args.exclusions, set(sources)) if args.exclusions else ({}, [])
    )

    total = covered = 0
    uncovered = []
    hidden = {}  # file -> number of uncovered lines the exclusions hide
    print(f"{'file':<32} {'covered/total':>14}  {'%':>6}")
    for rel in sorted(hits):
        # Merged by line number: cfg-gated code at the same line number on
        # different OSes is merged as one line.
        lines = {n: c for n, c in hits[rel].items() if n not in excluded.get(rel, ())}
        gone = [n for n, c in hits[rel].items() if n in excluded.get(rel, ()) and c == 0]
        if gone:
            hidden[rel] = len(gone)
        miss = sorted(n for n, c in lines.items() if c == 0)
        cov = len(lines) - len(miss)
        pct = 100.0 * cov / len(lines) if lines else 100.0
        print(f"{rel:<32} {f'{cov}/{len(lines)}':>14}  {pct:5.1f}%")
        total += len(lines)
        covered += cov
        uncovered.extend(f"{rel}:{n}" for n in miss)

    pct = 100.0 * covered / total if total else 100.0
    print(f"{'TOTAL':<32} {f'{covered}/{total}':>14}  {pct:5.1f}%")
    if hidden:
        print("\nLines hidden by exclusions (uncovered, excluded):")
        for rel in sorted(hidden):
            print(f"excluded: {rel} {hidden[rel]}")
    stale = []
    for n, rel, start, end in entries:
        suppressed = [
            ln for ln in range(start, end + 1) if hits.get(rel, {}).get(ln, 1) == 0
        ]
        if not suppressed:
            stale.append(f"{rel}:{start}-{end} (exclusions line {n})")
    if stale:
        print(f"\nWARNING: {len(stale)} stale exclusion(s) hide no uncovered line:")
        for item in stale:
            print(f"stale exclusion: {item}")
    if uncovered:
        print(f"\nUncovered lines ({len(uncovered)}):")
        for u in uncovered:
            print(u)
    if args.enforce and (uncovered or stale):
        print(f"\nFAIL: {len(uncovered)} uncovered line(s), {len(stale)} stale exclusion(s)")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
