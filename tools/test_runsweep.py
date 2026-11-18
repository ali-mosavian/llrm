"""runsweep: a wrong answer is WRONG, not ok; CRLF-writing programs (os_console) are not."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import runsweep  # noqa: E402


class Classify(unittest.TestCase):
    def test_wrong_output_is_reported_wrong(self):
        self.assertEqual(runsweep.classify(b"41\n", b"42\n"), "WRONG")

    def test_equal_output_is_ok(self):
        self.assertEqual(runsweep.classify(b"42\n", b"42\n"), "ok")

    def test_crlf_output_matches_lf_expectation(self):
        """os_console wrote ab\\r\\ncd\\r\\n against an LF .out and showed as WRONG in the first sweep."""
        self.assertEqual(runsweep.classify(b"ab\r\ncd\r\n", b"ab\ncd\n"), "ok")

    def test_report_exit_code(self):
        self.assertEqual(runsweep.report([("a.c", "-O1", "WRONG")]), 1)
        self.assertEqual(runsweep.report([("a.c", "-O1", "UNLINKED"), ("b.c", "-O1", "ok")]), 0)


class Runners(unittest.TestCase):
    def test_arm64_linux_runs_under_qemu(self):
        """arm64-linux had no runner: tests/run/c never executed there, so a loop-vectorize miscompile (bad=8352) went unseen."""
        self.assertEqual(runsweep.RUNNERS["arm64-linux"].launcher, ("qemu-aarch64",))
        self.assertEqual(runsweep.RUNNERS["x86_64-linux"].launcher, ())


if __name__ == "__main__":
    unittest.main()
