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

    def run_guard(self, base, bins=None, counts=None, base_argv="llc -O2 a.ll", head_argv="llc -O2 a.ll", extra=None):
        env = {k: v for k, v in os.environ.items() if not k.startswith(("LLRM_", "AB_ALLOW"))}
        env.update(AB_UPSTREAM="up", AB_BASE_ARGV=base_argv, AB_HEAD_ARGV=head_argv, **(extra or {}))
        for name in ("AB_BASE_ARGV", "AB_HEAD_ARGV"):
            if env[name] is None:
                del env[name]
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

    def test_an_argv_without_an_explicit_O_level_is_refused(self):
        """w12 compared llrm-wasm-llc (default -O0) with the pin's llc (-O2) on the same IR: 209268 vs 172620 B, a phantom gap."""
        for base_argv, head_argv in (("llc a.ll", "llc -O2 a.ll"), ("llc -O2 a.ll", "llc a.ll")):
            done = self.run_guard(self.c2, base_argv=base_argv, head_argv=head_argv)
            self.assertEqual(done.returncode, 2, (base_argv, head_argv))
            self.assertIn("explicit -O", done.stderr)

    def test_a_run_given_no_argv_is_refused(self):
        self.assertIn("argv", self.run_guard(self.c2, base_argv=None, head_argv=None).stderr)

    def test_both_argv_lines_are_printed_in_the_header(self):
        done = self.run_guard(self.c2, base_argv="llc -O2 a.ll", head_argv="llrm-wasm-llc -O2 a.ll")
        self.assertEqual(done.returncode, 0)
        self.assertIn("base argv: llc -O2 a.ll", done.stdout)
        self.assertIn("head argv: llrm-wasm-llc -O2 a.ll", done.stdout)

    def test_a_pipeline_switch_in_the_environment_is_refused(self):
        """LLRM_WASM_BACKEND_ONLY skipped the mid-end on one side; 40 minutes went to a second cause that was not there."""
        done = self.run_guard(self.c2, extra={"LLRM_WASM_BACKEND_ONLY": "1"})
        self.assertEqual(done.returncode, 2)
        self.assertIn("LLRM_WASM_BACKEND_ONLY", done.stderr)

    def test_a_tool_location_is_accepted_and_the_environment_is_printed(self):
        done = self.run_guard(self.c2, extra={"LLRM_PIN_CLANG": "/x/clang"})
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn("ab_env: env: LLRM_PIN_CLANG=/x/clang", done.stderr)

    def test_a_switch_named_in_ab_allow_env_is_accepted_and_printed(self):
        done = self.run_guard(self.c2, extra={"LLRM_NO_STACKS": "1", "AB_ALLOW_ENV": "LLRM_NO_STACKS"})
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn("LLRM_NO_STACKS=1", done.stderr)


if __name__ == "__main__":
    unittest.main()
