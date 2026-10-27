"""Tests for coverage_gate.py. Run: uv run python -m unittest scripts/test_coverage_gate.py"""

import io
import os
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))

GATE = SCRIPTS / "coverage_gate.py"

# Linux/macOS style: absolute path, line 3 not hit, line 7 never hit.
MAC = """TN:
SF:/Users/someone/proc/src/a.rs
DA:1,1
DA:2,4
DA:3,0
DA:5,0
DA:6,0
DA:7,0
end_of_record
"""

# Windows style: backslashes, line 3 hit here only. Also a dependency under
# the cargo registry whose path ends in /src/a.rs (must be ignored).
WIN = """TN:
SF:C:\\a\\proc\\proc\\src\\a.rs
DA:1,0
DA:2,0
DA:3,2
DA:5,0
DA:6,0
DA:7,0
end_of_record
SF:C:\\Users\\runner\\.cargo\\registry\\src\\index.crates.io-1949cf8c6b5b557f\\serde-1.0.0\\src\\a.rs
DA:1,0
DA:99,0
end_of_record
"""


class GateTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        (self.root / "src").mkdir()
        (self.root / "src" / "a.rs").write_text("// fixture\n")
        (self.root / "src" / "sub").mkdir()
        (self.root / "src" / "sub" / "a.rs").write_text("// fixture\n")
        self.mac = self.write("mac.info", MAC)
        self.win = self.write("win.info", WIN)

    def tearDown(self):
        self.tmp.cleanup()

    def write(self, name, text):
        p = self.root / name
        p.write_text(text)
        return str(p)

    def run_gate(self, *args):
        return subprocess.run(
            [sys.executable, str(GATE), "--repo-root", str(self.root), *args],
            capture_output=True,
            text=True,
        )

    def test_merge_counts_line_hit_in_either_file(self):
        r = self.run_gate(self.mac, self.win)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertNotIn("src/a.rs:3", r.stdout)
        self.assertIn("src/a.rs:5", r.stdout)
        self.assertIn("src/a.rs:7", r.stdout)

    def test_single_file_leaves_line_three_uncovered(self):
        r = self.run_gate(self.mac)
        self.assertIn("src/a.rs:3", r.stdout)

    def test_registry_dependency_is_ignored(self):
        r = self.run_gate(self.win)
        self.assertNotIn(":99", r.stdout)
        # the repo file has 6 DA lines in this record; the registry record adds none
        self.assertIn("src/a.rs", r.stdout)
        self.assertIn("1/6", r.stdout)

    def test_longest_repo_path_wins(self):
        sub = self.write(
            "sub.info", "SF:/x/proc/src/sub/a.rs\nDA:1,1\nend_of_record\n"
        )
        r = self.run_gate(sub)
        self.assertIn("src/sub/a.rs", r.stdout)
        self.assertIn("1/1", r.stdout)

    def test_unknown_file_is_ignored(self):
        other = self.write("o.info", "SF:/x/proc/src/nope.rs\nDA:1,0\nend_of_record\n")
        r = self.run_gate(other, "--enforce")
        self.assertEqual(r.returncode, 0, r.stdout)

    def test_exclusions_remove_lines(self):
        ex = self.write("ex.txt", "# header\nsrc/a.rs:5-6  not reachable\n")
        r = self.run_gate(self.mac, self.win, "--exclusions", ex)
        self.assertNotIn("src/a.rs:5", r.stdout)
        self.assertNotIn("src/a.rs:6", r.stdout)
        self.assertIn("src/a.rs:7", r.stdout)

    def test_enforce_fails_while_line_seven_uncovered(self):
        r = self.run_gate(self.mac, self.win, "--enforce")
        self.assertEqual(r.returncode, 1)
        self.assertIn("src/a.rs:7", r.stdout)

    def test_enforce_passes_when_everything_covered_or_excluded(self):
        ex = self.write("ex.txt", "src/a.rs:5-7  fixture\n")
        r = self.run_gate(self.mac, self.win, "--exclusions", ex, "--enforce")
        self.assertEqual(r.returncode, 0, r.stdout)

    def test_report_only_exits_zero(self):
        r = self.run_gate(self.mac)
        self.assertEqual(r.returncode, 0)

    def test_custom_cargo_home_registry_is_ignored(self):
        # CARGO_HOME=/out/cargo-home (coverage-local.sh): no ".cargo" segment
        dep = self.write(
            "dep.info",
            "SF:/out/cargo-home/registry/src/index.crates.io-1949cf8c6b5b557f/"
            "serde-1.0.228/src/a.rs\nDA:1,0\nDA:2,0\nend_of_record\n"
            "SF:/out/cargo-home/git/checkouts/foo-1234/abcdef/src/a.rs\n"
            "DA:3,0\nend_of_record\n"
            "SF:/rustc/abc123/library/core/src/a.rs\nDA:4,0\nend_of_record\n"
            "SF:/work/src/a.rs\nDA:1,1\nend_of_record\n",
        )
        r = self.run_gate(dep, "--enforce")
        self.assertEqual(r.returncode, 0, r.stdout)
        self.assertIn("1/1", r.stdout)

    def test_exclusion_without_reason_is_rejected(self):
        ex = self.write("ex.txt", "src/a.rs:5-6\n")
        r = self.run_gate(self.mac, "--exclusions", ex)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("reason", r.stderr)

    def test_exclusion_of_unknown_file_or_reversed_range_is_rejected(self):
        ex = self.write("ex.txt", "src/nope.rs:1-2  gone\n")
        r = self.run_gate(self.mac, "--exclusions", ex)
        self.assertNotEqual(r.returncode, 0)
        ex = self.write("ex2.txt", "src/a.rs:6-5  backwards\n")
        r = self.run_gate(self.mac, "--exclusions", ex)
        self.assertNotEqual(r.returncode, 0)

    def test_stale_exclusion_warns_and_fails_under_enforce(self):
        # line 1 is covered in the merged data, so excluding it hides nothing
        ex = self.write("ex.txt", "src/a.rs:5-7  real\nsrc/a.rs:1  stale\n")
        r = self.run_gate(self.mac, self.win, "--exclusions", ex)
        self.assertEqual(r.returncode, 0, r.stdout)
        self.assertIn("stale exclusion", r.stdout)
        self.assertIn("src/a.rs:1", r.stdout)
        r = self.run_gate(self.mac, self.win, "--exclusions", ex, "--enforce")
        self.assertEqual(r.returncode, 1)

    def test_stale_exclusion_for_file_without_data(self):
        ex = self.write("ex.txt", "src/sub/a.rs:1-2  no data for this file\n")
        r = self.run_gate(self.mac, "--exclusions", ex)
        self.assertIn("stale exclusion", r.stdout)

    def test_report_counts_excluded_lines_per_file(self):
        ex = self.write("ex.txt", "src/a.rs:5-6  two lines\n")
        r = self.run_gate(self.mac, self.win, "--exclusions", ex)
        self.assertIn("excluded: src/a.rs 2", r.stdout)


if __name__ == "__main__":
    unittest.main()
