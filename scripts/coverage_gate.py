#!/usr/bin/env python3
"""Line-coverage gate for proc.

Usage: coverage_gate.py <lcov>... [--exclusions FILE] [--enforce]
       [--require NAME]... [--repo-root DIR]

Each lcov `SF:` path is mapped to a repo file by comparing it with the repo's
real src/**/*.rs files (backslashes normalised to '/'; the longest repo path
that the SF path ends with wins). Paths that match no repo file, and anything
under a cargo registry/git checkout (any CARGO_HOME) or the rustc sources, are
ignored.
Line hits are merged across all lcov files (covered if hit in any). Lines in
the exclusions file (`src/path.rs:START-END  reason`, reason required, `#` comments) are dropped;
an exclusion that hides no uncovered line is reported as stale (a failure under
--enforce).
Prints per-file covered/total and every uncovered `src/path.rs:LINE`. Right after
the merged TOTAL, when several lcov files are given, it prints each input lcov's
own total, labelled by its file name (for information only: the merged result is
the only rule; the percentage is cut, not rounded, so one missed line never
reads as 100.0%). With --enforce, exits 1 if any line is uncovered or a stale exclusion remains.
--enforce without any --require is an error. Also under --enforce: each --require NAME must match (as a substring of the
file name) exactly one lcov file and every lcov file must match one, so a
failed OS job cannot slip through as a partial merge; and a src file with no
DA record at all fails unless it declares no `fn` outside // comments (module
files of re-exports have nothing to instrument; a file that was never
compiled does).
"""

import argparse
import math
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


def declares_fn(path):
    code = "".join(
        line
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines(True)
        if not line.lstrip().startswith("//")
    )
    return re.search(r"\bfn\b", code) is not None


def check_required(lcov, required):
    """Problems with the set of lcov files given against the --require names."""
    names = [Path(p).name for p in lcov]
    problems = []
    for want in required:
        n = sum(want in name for name in names)
        if n == 0:
            problems.append(f"missing lcov: no file name contains '{want}'")
        elif n > 1:
            problems.append(f"'{want}' matches {n} lcov files, expected 1")
    for name in names:
        if not any(want in name for want in required):
            problems.append(f"unexpected lcov: {name} matches no --require name")
    return problems


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("lcov", nargs="+")
    ap.add_argument("--exclusions")
    ap.add_argument("--enforce", action="store_true")
    ap.add_argument(
        "--require",
        action="append",
        default=[],
        metavar="NAME",
        help="under --enforce, NAME must match exactly one lcov file name (repeatable)",
    )
    ap.add_argument("--repo-root", default=str(Path(__file__).resolve().parent.parent))
    args = ap.parse_args()
    if args.enforce and not args.require:
        ap.error("--enforce needs at least one --require, or a missing OS's coverage goes unnoticed")

    sources = repo_sources(Path(args.repo_root))
    hits = {}
    per_file_hits = []  # (lcov path, its own hits)
    for path in args.lcov:
        if not Path(path).is_file():
            # an unmatched shell glob arrives as the literal pattern
            sys.exit(f"no lcov file at {path}: did every coverage job fail?")
        own = {}
        parse_lcov(path, sources, own)
        per_file_hits.append((path, own))
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
    if len(per_file_hits) > 1:
        print("\nPer lcov file (information only; the merged TOTAL is the rule):")
        for path, own in per_file_hits:
            own_total = own_covered = 0
            for rel, lines in own.items():
                kept = [c for n, c in lines.items() if n not in excluded.get(rel, ())]
                own_total += len(kept)
                own_covered += sum(1 for c in kept if c > 0)
            # cut to one decimal, never rounded up: 8347/8348 must not read 100.0%
            own_pct = (
                math.floor(1000.0 * own_covered / own_total) / 10 if own_total else 100.0
            )
            label = Path(path).name
            print(f"{label:<32} {f'{own_covered}/{own_total}':>14}  {own_pct:5.1f}%")
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
    root = Path(args.repo_root)
    nodata = [rel for rel in sources if rel not in hits and declares_fn(root / rel)]
    if nodata:
        print(f"\nSource files with code and no coverage data ({len(nodata)}):")
        for rel in sorted(nodata):
            print(f"no coverage data: {rel}")
    problems = check_required(args.lcov, args.require) if args.require else []
    for item in problems:
        print(item)
    if args.enforce and (uncovered or stale or nodata or problems):
        print(
            f"\nFAIL: {len(uncovered)} uncovered line(s), {len(stale)} stale "
            f"exclusion(s), {len(nodata)} file(s) without data, "
            f"{len(problems)} lcov set problem(s)"
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
