"""A compile limit is CPU seconds: on a core shared with busy processes the same command takes several times as long
on the wall clock, and a limit in wall seconds stopped compiles that were fine (test_loops' 10 s at load 70)."""

import os
import shutil
import subprocess
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "loops"))

import build  # noqa: E402
import cpulimit  # noqa: E402

SPIN = "while True: pass"
# 0.4 s of CPU, however long the wall clock takes to give it.
WORK = "import time\nwhile time.process_time() < 0.4: pass\nprint('done')"


@unittest.skipUnless(shutil.which("taskset"), "needs taskset to share one core")
class ContentionTests(unittest.TestCase):
    def setUp(self):
        core = str(min(os.sched_getaffinity(0)))
        self.pinned = ["taskset", "-c", core, sys.executable]
        self.hogs = [subprocess.Popen([*self.pinned, "-c", SPIN]) for _ in range(4)]
        self.addCleanup(lambda: [hog.kill() for hog in self.hogs])

    def test_a_command_sharing_a_core_with_busy_ones_is_stopped_by_a_wall_limit_and_not_by_a_cpu_one(self):
        # The old limit: five processes on a core give the command a fifth of it, 2 s of wall for 0.4 s of CPU.
        with self.assertRaises(subprocess.TimeoutExpired):
            subprocess.run([*self.pinned, "-c", WORK], capture_output=True, text=True, timeout=1.2)
        done = cpulimit.run([*self.pinned, "-c", WORK], 3, capture_output=True, text=True)
        self.assertEqual((done.returncode, done.stdout.strip()), (0, "done"), done.stderr)

    def test_the_loops_build_limit_is_cpu_seconds(self):
        # build._run(timeout=1) raised CompileError("did not finish in 1s") here while it was wall clock.
        self.assertEqual(build._run([*self.pinned, "-c", WORK], timeout=1).strip(), "done")


class LimitTests(unittest.TestCase):
    def test_a_command_that_burns_its_cpu_limit_is_stopped(self):
        with self.assertRaises(subprocess.TimeoutExpired):
            cpulimit.run([sys.executable, "-c", SPIN], 1, capture_output=True, text=True)

    def test_a_command_that_only_waits_is_not_stopped_by_the_cpu_limit(self):
        done = cpulimit.run([sys.executable, "-c", "import time; time.sleep(2)"], 1, capture_output=True, text=True)
        self.assertEqual(done.returncode, 0)

    def test_a_failing_command_is_returned_not_raised(self):
        done = cpulimit.run([sys.executable, "-c", "import sys; sys.exit(3)"], 5, capture_output=True, text=True)
        self.assertEqual(done.returncode, 3)


if __name__ == "__main__":
    unittest.main()


class MemoryLimitTests(unittest.TestCase):
    def test_a_child_allocating_beyond_the_limit_is_stopped_and_reported_as_a_memory_limit(self):
        """datetime.c compiled with the translated JumpThreading grew to 50 GB before a timeout stopped it: a runaway must die at its limit and say so."""
        with self.assertRaises(cpulimit.MemoryLimit):
            cpulimit.run([sys.executable, "-c", "x = bytearray(900 * 1024 * 1024)"], 60, memory_kb=300_000, capture_output=True, text=True)
        done = cpulimit.run([sys.executable, "-c", "x = bytearray(10 * 1024 * 1024); print('ok')"], 60, memory_kb=300_000, capture_output=True, text=True)
        self.assertEqual(done.stdout.strip(), "ok")
