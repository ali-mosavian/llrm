"""ab_guard.sh refuses a stale base: A/B numbers measured against it are confounded."""

import os
import subprocess
import tempfile
import unittest

GUARD = os.path.join(os.path.dirname(__file__), "ab_guard.sh")


def git(repo, *args):
    return subprocess.run(
        ["git", "-C", repo, "-c", "user.name=t", "-c", "user.email=t@t", *args],
        check=True, capture_output=True, text=True,
    ).stdout.strip()


class AbGuard(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        r = self.repo = self.dir.name
        git(r, "init", "-q", "-b", "up")
        for n in "123":
            open(os.path.join(r, "f"), "w").write(n)
            git(r, "add", "f")
            git(r, "commit", "-q", "-m", n)
        self.c1, self.c2, self.c3 = git(r, "rev-list", "--reverse", "up").split()
        git(r, "checkout", "-q", "-b", "head", self.c2)
        open(os.path.join(r, "g"), "w").write("x")
        git(r, "add", "g")
        git(r, "commit", "-q", "-m", "head")
        self.bins = []
        for name in ("b", "h"):
            p = os.path.join(r, "bin." + name)
            open(p, "w").write(name)
            self.bins.append(p)
        self.counts = os.path.join(r, "counts")
        open(self.counts, "w").write("1 2\n")

    def run_guard(self, base, bins=None, counts=None):
        env = dict(os.environ, AB_UPSTREAM="up")
        b, h = bins or self.bins
        return subprocess.run(
            [GUARD, self.repo, base, "head", b, h, counts or self.counts],
            env=env, capture_output=True, text=True,
        )

    def test_the_newest_upstream_commit_head_contains_is_accepted(self):
        self.assertEqual(self.run_guard(self.c2).returncode, 0)

    def test_a_stale_base_older_than_the_merge_base_is_refused(self):
        """Base c1 while head sits on c2: the A/B blamed c2's change on head (confounded)."""
        done = self.run_guard(self.c1)
        self.assertEqual(done.returncode, 2)
        self.assertIn("stale base", done.stderr)

    def test_a_base_head_does_not_contain_is_refused(self):
        self.assertEqual(self.run_guard(self.c3).returncode, 2)

    def test_identical_binaries_are_refused(self):
        self.assertIn("identical", self.run_guard(self.c2, [self.bins[0], self.bins[0]]).stderr)

    def test_an_empty_counts_file_is_refused(self):
        empty = os.path.join(self.repo, "empty")
        open(empty, "w").close()
        self.assertIn("empty", self.run_guard(self.c2, counts=empty).stderr)


if __name__ == "__main__":
    unittest.main()
