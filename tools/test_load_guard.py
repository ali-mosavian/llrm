"""load_guard.sh stops a command at high load, resumes it at low load, and never leaves its group stopped."""

import os
import re
import signal
import subprocess
import tempfile
import time
import unittest

GUARD = os.path.join(os.path.dirname(os.path.abspath(__file__)), "load_guard.sh")


def state(pid):
    try:
        return open(f"/proc/{pid}/stat").read().rsplit(")", 1)[1].split()[0]
    except OSError:
        return "gone"


def until(check, seconds=8):
    end = time.time() + seconds
    while time.time() < end:
        if check():
            return True
        time.sleep(0.05)
    return False


class LoadGuard(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.load = os.path.join(self.dir.name, "load")
        self.count = os.path.join(self.dir.name, "count")
        self.set_load(1.0, 1.0)
        self.guard = subprocess.Popen(
            [GUARD, "bash", "-c", f"while :; do echo x >> {self.count}; sleep 0.05; done"],
            env={**os.environ, "LG_LOAD_FILE": self.load, "LG_POLL": "0.1"}, stderr=subprocess.PIPE, text=True,
        )
        self.assertTrue(until(lambda: os.path.exists(self.count)))

    def tearDown(self):
        if self.guard.poll() is None:
            self.guard.terminate()
            self.guard.wait(timeout=5)
        self.dir.cleanup()

    def set_load(self, one, five):
        open(self.load, "w").write(f"{one} {five} 1.0 1/1 1\n")

    def lines(self):
        return len(open(self.count).read().splitlines())

    def group(self):
        """The command's pid: the child of the guard, its own group leader."""
        out = subprocess.run(["pgrep", "-P", str(self.guard.pid), "-f", "while :"], capture_output=True, text=True).stdout.split()
        return int(out[0])

    def test_high_load_stops_the_group_and_low_load_resumes_it(self):
        pgid = self.group()
        self.set_load(30.0, 30.0)
        self.assertTrue(until(lambda: state(pgid) == "T"), state(pgid))
        frozen = self.lines()
        time.sleep(0.5)
        self.assertEqual(self.lines(), frozen, "the command ran while stopped")
        self.set_load(20.0, 20.0)
        self.assertTrue(until(lambda: state(pgid) != "T"))
        self.assertTrue(until(lambda: self.lines() > frozen))

    def test_a_killed_wrapper_leaves_nothing_stopped(self):
        """A guard left a build stopped for 67 minutes; the watchdog continues the group when the wrapper dies."""
        pgid = self.group()
        self.set_load(30.0, 30.0)
        self.assertTrue(until(lambda: state(pgid) == "T"))
        os.kill(self.guard.pid, signal.SIGKILL)
        self.guard.wait()  # reaped, as a shell would: a zombie still answers kill -0
        try:
            self.assertTrue(until(lambda: state(pgid) != "T"), "the group stayed in state T")
        finally:
            os.killpg(pgid, signal.SIGCONT)
            os.killpg(pgid, signal.SIGKILL)

    def test_every_stop_and_resume_line_is_stamped_with_date_u(self):
        pgid = self.group()
        self.set_load(30.0, 30.0)
        self.assertTrue(until(lambda: state(pgid) == "T"))
        self.set_load(10.0, 10.0)
        self.assertTrue(until(lambda: state(pgid) != "T"))
        self.guard.terminate()
        err = self.guard.communicate(timeout=5)[1]
        for word in ("stop", "resume"):
            self.assertRegex(err, rf"load_guard: \d\d:\d\dZ {word} pgid {pgid}")


if __name__ == "__main__":
    unittest.main()
