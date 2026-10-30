#!/usr/bin/env python3
"""Write the website's changelog (docs/changelog.md) from CHANGELOG.md.

CHANGELOG.md is the one place a release's notes are written. The website shows
the same entries with its own headings (`## 2.0.2 — 2026-10-30`, `**Fixed**`
for `## [2.0.2] - 2026-10-30`, `### Fixed`), between two marker comments.
Below the markers the page keeps its own, rewritten history of the older
releases, which this script leaves alone: it writes every release newer than
the newest one there. `[Unreleased]` is not on the website.

    python3 scripts/changelog_site.py           # rewrite docs/changelog.md
    python3 scripts/changelog_site.py --check   # exit 1 if it is out of date (CI)
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "CHANGELOG.md"
SITE = ROOT / "docs" / "changelog.md"
BEGIN = "<!-- From CHANGELOG.md by scripts/changelog_site.py: edit CHANGELOG.md, not this part -->"
END = "<!-- End of the part from CHANGELOG.md -->"

RELEASE = re.compile(r"^## \[(\d+\.\d+\.\d+)\] - (\S+)$", re.M)
SITE_RELEASE = re.compile(r"^## (\d+\.\d+\.\d+) — ", re.M)


def version_key(version):
    return tuple(int(part) for part in version.split("."))


def releases(changelog):
    """Each release in CHANGELOG.md, newest first: (version, date, body)"""
    found = []
    matches = list(RELEASE.finditer(changelog))
    for i, match in enumerate(matches):
        end = matches[i + 1].start() if i + 1 < len(matches) else len(changelog)
        body = changelog[match.end():end]
        # the compare links at the bottom of the file follow the oldest release
        body = re.split(r"^\[[^\]]+\]: ", body, maxsplit=1, flags=re.M)[0]
        found.append((match.group(1), match.group(2), body.strip("\n")))
    return found


def site_section(version, date, body):
    """One release as the website writes it"""
    body = re.sub(r"^### (\w[\w ]*)$", r"**\1**", body, flags=re.M)
    return f"## {version} — {date}\n\n{body}\n"


def render(changelog, site):
    """The website page with its part from CHANGELOG.md rewritten"""
    head, rest = site.split(BEGIN + "\n", 1)
    _, tail = rest.split(END + "\n", 1)
    kept = SITE_RELEASE.search(tail)
    newest_kept = version_key(kept.group(1)) if kept else (0, 0, 0)
    sections = [
        site_section(version, date, body)
        for version, date, body in releases(changelog)
        if version_key(version) > newest_kept
    ]
    return head + BEGIN + "\n\n" + "\n".join(sections) + "\n" + END + "\n" + tail


def main(argv):
    current = SITE.read_text()
    wanted = render(SOURCE.read_text(), current)
    if "--check" in argv:
        if wanted != current:
            print(
                "docs/changelog.md is out of date with CHANGELOG.md: "
                "run python3 scripts/changelog_site.py",
                file=sys.stderr,
            )
            return 1
        return 0
    SITE.write_text(wanted)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
