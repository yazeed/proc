"""Self-test for changelog_site.py (python3 -m unittest scripts/test_changelog_site.py)"""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import changelog_site as site  # noqa: E402

CHANGELOG = """# Changelog

## [Unreleased]

### Fixed

- not released yet

## [2.1.0] - 2026-11-02

### Added

- a new thing

### Fixed

- a fix

## [2.0.0] - 2026-10-28

### Changed

- the old release

[Unreleased]: https://example.com/compare/v2.1.0...HEAD
[2.1.0]: https://example.com/compare/v2.0.0...v2.1.0
"""

PAGE = (
    "---\ntitle: Changelog\n---\n\n"
    + site.BEGIN
    + "\n\n## 1.9.0 — stale\n\n"
    + site.END
    + "\n\n## 2.0.0 — 2026-10-28\n\nThe website's own words for 2.0.0.\n"
)


class Render(unittest.TestCase):
    def test_writes_newer_releases_with_the_websites_headings(self):
        page = site.render(CHANGELOG, PAGE)
        self.assertIn("## 2.1.0 — 2026-11-02\n\n**Added**\n\n- a new thing\n\n**Fixed**", page)
        self.assertNotIn("not released yet", page)
        self.assertNotIn("https://example.com", page)
        self.assertNotIn("1.9.0 — stale", page)
        # the website's own history below the markers is kept, not replaced
        self.assertIn("The website's own words for 2.0.0.", page)
        self.assertNotIn("the old release", page)

    def test_a_second_run_changes_nothing(self):
        once = site.render(CHANGELOG, PAGE)
        self.assertEqual(site.render(CHANGELOG, once), once)


if __name__ == "__main__":
    unittest.main()
