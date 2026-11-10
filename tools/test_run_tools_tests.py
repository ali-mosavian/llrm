"""run_tools_tests.py: a failing test, an import error, a file with no tests and a pytest file with no uv are each exit 1."""

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

RUNNER = Path(__file__).resolve().parent / "run_tools_tests.py"


class Runner(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.root = Path(self.dir.name)

    def tearDown(self):
        self.dir.cleanup()

    def run_on(self, source, path_env=None):
        (self.root / "test_x.py").write_text(source)
        env = {**os.environ, **({"PATH": path_env} if path_env is not None else {})}
        return subprocess.run([sys.executable, "-I", str(RUNNER), "--root", str(self.root)], capture_output=True, text=True, env=env)

    def test_plain_and_unittest_tests_pass(self):
        done = self.run_on("import unittest\ndef test_a():\n    assert 1\nclass T(unittest.TestCase):\n    def test_b(self):\n        self.assertTrue(1)\n")
        self.assertEqual(done.returncode, 0, done.stdout)
        self.assertIn("2 passed, 0 failed", done.stdout)

    def test_a_failing_test_exits_1(self):
        """A runner that printed FAIL and exited 0 would let a vouch pass on a red tools suite."""
        done = self.run_on("def test_a():\n    assert 0\n")
        self.assertEqual(done.returncode, 1, done.stdout)
        self.assertIn("FAIL", done.stdout)

    def test_an_import_error_is_a_fail_not_a_skip(self):
        done = self.run_on("import no_such_module_here\ndef test_a():\n    pass\n")
        self.assertEqual(done.returncode, 1, done.stdout)
        self.assertIn("import error", done.stdout)

    def test_a_file_with_no_tests_is_a_fail(self):
        done = self.run_on("x = 1\n")
        self.assertEqual(done.returncode, 1, done.stdout)

    def test_a_pytest_file_without_uv_is_needs_pytest_fail(self):
        done = self.run_on("import pytest\ndef test_a(tmp_path):\n    pass\n", path_env="/nonexistent")
        self.assertEqual(done.returncode, 1, done.stdout)
        self.assertIn("NEEDS-PYTEST", done.stdout)


if __name__ == "__main__":
    unittest.main()
