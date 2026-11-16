"""An A/B whose base is older than the merge-base of the PR head with tip measured a state that never landed: d99 (11-16) read +740 B on one
x86_64 cell from a pair built before the PR's last merge of tip, and two rows were booked on it."""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import stamp  # noqa: E402


def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True, check=True).stdout.strip()


def commit(repo, path, text):
    Path(repo, path).parent.mkdir(parents=True, exist_ok=True)
    Path(repo, path).write_text(text)
    git(repo, "add", path)
    git(repo, "commit", "-q", "-m", text)
    return git(repo, "rev-parse", "HEAD")


def dist(root, name, sha):
    out = Path(root, name)
    out.mkdir()
    binary = out / "llrm-llc"
    binary.write_text(f"#!/bin/sh\necho 'llrm-llc {sha}'\n")
    binary.chmod(0o755)
    return out


class PairTests(unittest.TestCase):
    def repo(self, tmp):
        repo = Path(tmp, "repo")
        repo.mkdir()
        git(repo, "init", "-q", "-b", "main")
        git(repo, "config", "user.email", "t@t")
        git(repo, "config", "user.name", "t")
        old = commit(repo, "crates/a", "old")
        git(repo, "checkout", "-q", "-b", "pr")
        feature = commit(repo, "crates/f", "feature")
        git(repo, "checkout", "-q", "main")
        tip = commit(repo, "crates/b", "tip")
        git(repo, "checkout", "-q", "pr")
        git(repo, "merge", "-q", "--no-edit", "main")
        return repo, old, feature, tip, git(repo, "rev-parse", "HEAD")

    def test_a_base_older_than_the_merge_base_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo, old, feature, tip, head = self.repo(tmp)
            with self.assertRaises(SystemExit) as caught:
                stamp.require_pair(dist(tmp, "base", old), dist(tmp, "new", head), head, tip, repo)
            self.assertIn("base is stale", str(caught.exception))

    def test_a_new_built_before_the_last_merge_of_tip_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo, old, feature, tip, head = self.repo(tmp)
            with self.assertRaises(SystemExit) as caught:
                stamp.require_pair(dist(tmp, "base", tip), dist(tmp, "new", feature), head, tip, repo)
            self.assertIn("measured another tree", str(caught.exception))

    def test_base_at_the_merge_base_and_new_at_head_pass(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo, old, feature, tip, head = self.repo(tmp)
            stamp.require_pair(dist(tmp, "base", tip), dist(tmp, "new", head), head, tip, repo)


if __name__ == "__main__":
    unittest.main()
