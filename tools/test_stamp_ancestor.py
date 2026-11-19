"""A dist whose compiler was built from a commit that is not an ancestor of the commit a table names measured another compiler: a
hybrid dist carried `llrm-nib` from before #6966 beside a newer `llrm-llc`, only the backend's stamp was read, and every Nib number
of a day predated a fix it was meant to show (11-05)."""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import stamp  # noqa: E402


def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True, check=True).stdout.strip()


def binary(dist, name, sha):
    path = Path(dist, name)
    path.write_text(f"#!/bin/sh\necho '{name} {sha}'\n")
    path.chmod(0o755)


class AncestorTests(unittest.TestCase):
    def repo(self, tmp):
        repo = Path(tmp, "repo")
        repo.mkdir()
        git(repo, "init", "-q", "-b", "main")
        git(repo, "config", "user.email", "t@t")
        git(repo, "config", "user.name", "t")
        for text in ("a", "b"):
            (repo / "f").write_text(text)
            git(repo, "add", "f")
            git(repo, "commit", "-q", "-m", text)
        old, new = git(repo, "rev-parse", "HEAD~1"), git(repo, "rev-parse", "HEAD")
        git(repo, "checkout", "-q", "-b", "side", old)
        (repo / "g").write_text("side")
        git(repo, "add", "g")
        git(repo, "commit", "-q", "-m", "side")
        return repo, old, new, git(repo, "rev-parse", "HEAD")

    def test_a_compiler_from_another_branch_is_refused_though_the_backend_is_current(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo, old, new, side = self.repo(tmp)
            dist = Path(tmp, "dist")
            dist.mkdir()
            binary(dist, "llrm-llc", new)
            binary(dist, "llrm-nib", side)
            with self.assertRaises(SystemExit) as caught:
                stamp.require_ancestor(dist, new, repo)
            self.assertIn("llrm-nib", str(caught.exception))

    def test_compilers_built_from_ancestors_pass_and_are_recorded(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo, old, new, side = self.repo(tmp)
            dist = Path(tmp, "dist")
            dist.mkdir()
            binary(dist, "llrm-llc", new)
            binary(dist, "llrm-nib", old)
            recorded = stamp.require_ancestor(dist, new, repo)
            self.assertEqual({name: sha for name, (sha, _) in recorded.items()}, {"llrm-llc": new, "llrm-nib": old})


if __name__ == "__main__":
    unittest.main()
