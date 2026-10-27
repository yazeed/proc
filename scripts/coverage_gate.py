#!/usr/bin/env python3
"""Line-coverage gate for proc.

Usage: coverage_gate.py <lcov>... [--exclusions FILE] [--enforce] [--repo-root DIR]

Each lcov `SF:` path is mapped to a repo file by comparing it with the repo's
real src/**/*.rs files (backslashes normalised to '/'; the longest repo path
that the SF path ends with wins). Paths that match no repo file, and anything
under a cargo registry/git checkout or the rustc sources, are ignored.
Line hits are merged across all lcov files (covered if hit in any). Lines in
the exclusions file (`src/path.rs:START-END  reason`, `#` comments) are dropped.
Prints per-file covered/total and every uncovered `src/path.rs:LINE`; with
--enforce, exits 1 if any line is uncovered.
"""

import argparse
import re
import sys
from pathlib import Path

DEPENDENCY_MARKERS = ("/.cargo/registry/", "/.cargo/git/", "/rustc/")


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


def parse_exclusions(path):
    excluded = {}
    pattern = re.compile(r"^(\S+?):(\d+)(?:-(\d+))?(?:\s|$)")
    for n, raw in enumerate(Path(path).read_text().splitlines(), 1):
        text = raw.strip()
        if not text or text.startswith("#"):
            continue
        m = pattern.match(text)
        if not m:
            sys.exit(f"{path}:{n}: expected 'src/path.rs:START-END  reason', got: {text}")
        start = int(m.group(2))
        end = int(m.group(3) or start)
        excluded.setdefault(m.group(1), set()).update(range(start, end + 1))
    return excluded


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
    excluded = parse_exclusions(args.exclusions) if args.exclusions else {}

    total = covered = 0
    uncovered = []
    print(f"{'file':<32} {'covered/total':>14}  {'%':>6}")
    for rel in sorted(hits):
        lines = {n: c for n, c in hits[rel].items() if n not in excluded.get(rel, ())}
        miss = sorted(n for n, c in lines.items() if c == 0)
        cov = len(lines) - len(miss)
        pct = 100.0 * cov / len(lines) if lines else 100.0
        print(f"{rel:<32} {f'{cov}/{len(lines)}':>14}  {pct:5.1f}%")
        total += len(lines)
        covered += cov
        uncovered.extend(f"{rel}:{n}" for n in miss)

    pct = 100.0 * covered / total if total else 100.0
    print(f"{'TOTAL':<32} {f'{covered}/{total}':>14}  {pct:5.1f}%")
    if uncovered:
        print(f"\nUncovered lines ({len(uncovered)}):")
        for u in uncovered:
            print(u)
    if args.enforce and uncovered:
        print(f"\nFAIL: {len(uncovered)} uncovered line(s)")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
