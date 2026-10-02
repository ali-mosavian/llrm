"""`uv run --project tools python -m unittest discover -s tools/bench`"""

import json
import sys
import tempfile
import subprocess
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import history  # noqa: E402


class LineTests(unittest.TestCase):
    def test_one_line_per_measurement_with_the_commit_and_the_counts(self):
        measured = {"sieve/c/O2": {"instructions": 12109, "memory_operands": 2728, "ms": 0.17}, "parity/scalar/nib/Os": {"error": "x", "known": "#1"}}
        lines = [json.loads(one) for one in history.lines_for("abc", "2026-10-03T00:00:00+00:00", "subject", measured)]
        self.assertEqual(len(lines), 2)
        self.assertEqual((lines[0]["benchmark"], lines[0]["language"], lines[0]["opt"], lines[0]["known"]), ("parity/scalar", "nib", "Os", "#1"))
        self.assertEqual((lines[1]["benchmark"], lines[1]["language"], lines[1]["opt"], lines[1]["instructions"]), ("sieve", "c", "O2", 12109))

    def test_a_benchmark_under_parity_keeps_its_whole_name(self):
        """parity/scalar split on the last two slashes only: its language is nib, not scalar."""
        line = json.loads(history.lines_for("a", "d", "s", {"parity/scalar/nib/O2": {"instructions": 1}})[0])
        self.assertEqual((line["benchmark"], line["language"]), ("parity/scalar", "nib"))


class BranchTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.repo = Path(self.temp.name) / "repo"
        self.repo.mkdir()
        run = lambda *a: subprocess.run(["git", *a], cwd=self.repo, check=True, capture_output=True)  # noqa: E731
        run("init", "-q", "-b", "main")
        run("config", "user.email", "t@example.com")
        run("config", "user.name", "t")
        (self.repo / "file").write_text("x")
        run("add", "file")
        run("commit", "-q", "-m", "first")
        self.saved, history.ROOT = history.ROOT, self.repo

    def tearDown(self):
        history.ROOT = self.saved
        self.temp.cleanup()

    def test_the_branch_is_an_orphan_that_holds_only_the_results(self):
        tree = Path(self.temp.name) / "tree"
        history.history_worktree(tree)
        history.append(tree, ["{\"commit\": \"a\"}"], "bench: a")
        files = subprocess.run(["git", "ls-tree", "-r", "--name-only", history.BRANCH], cwd=self.repo, capture_output=True, text=True).stdout.split()
        self.assertEqual(files, ["results.jsonl"])
        parents = subprocess.run(["git", "rev-list", "--parents", "-n1", history.BRANCH], cwd=self.repo, capture_output=True, text=True).stdout.split()
        self.assertEqual(len(parents), 1, "an orphan commit has no parent")

    def test_a_tree_an_interrupted_run_left_behind_is_replaced(self):
        """Killing a backfill left tree-21428b83 behind and the next run died on `already exists`."""
        tree = Path(self.temp.name) / "tree"
        history.fresh_tree(tree, "main")
        (tree / "stale").write_text("x")
        history.fresh_tree(tree, "main")
        self.assertTrue((tree / "file").exists())
        self.assertFalse((tree / "stale").exists())

    def test_a_recorded_commit_is_recognised_on_the_next_run(self):
        tree = Path(self.temp.name) / "tree"
        history.history_worktree(tree)
        history.append(tree, history.lines_for("a" * 40, "d", "s", {"sieve/c/O2": {"instructions": 1}}), "bench: a")
        history.git("worktree", "remove", "--force", str(tree))
        again = Path(self.temp.name) / "again"
        history.history_worktree(again)
        self.assertEqual(history.recorded(again / history.RESULTS), {"a" * 40})


if __name__ == "__main__":
    unittest.main()
