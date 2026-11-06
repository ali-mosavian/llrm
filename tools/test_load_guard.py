"""load_guard.sh admits a bounded number of jobs, stops a job's whole process tree at high load, resumes exactly it, and never leaves it stopped."""

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


def descendants(root):
    """Every live pid below `root`."""
    table = {}
    for name in os.listdir("/proc"):
        if name.isdigit():
            try:
                table[int(name)] = int(open(f"/proc/{name}/stat").read().rsplit(")", 1)[1].split()[1])
            except (OSError, IndexError):
                pass
    found, frontier = [], [root]
    while frontier:
        parent = frontier.pop()
        for pid, ppid in table.items():
            if ppid == parent and pid not in found:
                found.append(pid)
                frontier.append(pid)
    return found


class Env:
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.load = os.path.join(self.dir.name, "load")
        self.count = os.path.join(self.dir.name, "count")
        self.slots = os.path.join(self.dir.name, "slots")
        self.set_load(1.0, 1.0)
        self.guards = []

    def tearDown(self):
        for guard in self.guards:
            if guard.poll() is None:
                guard.terminate()
                guard.wait(timeout=5)
        self.dir.cleanup()

    def set_load(self, one, five):
        open(self.load, "w").write(f"{one} {five} 1.0 1/1 1\n")

    def lines(self):
        return len(open(self.count).read().splitlines())

    def start(self, *command, **env):
        guard = subprocess.Popen(
            [GUARD, *command],
            env={**os.environ, "LG_LOAD_FILE": self.load, "LG_POLL": "0.1", "LG_SLOT_DIR": self.slots, **env},
            stderr=subprocess.PIPE, text=True,
        )
        self.guards.append(guard)
        return guard

    def counter(self):
        return ["bash", "-c", f"while :; do echo x >> {self.count}; sleep 0.05; done"]


class Stopping(Env, unittest.TestCase):
    def setUp(self):
        super().setUp()
        self.guard = self.start(*self.counter())
        self.assertTrue(until(lambda: os.path.exists(self.count)))

    def group(self):
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
            self.assertRegex(err, rf"load_guard: \d\d:\d\dZ {word} groups .*\b{pgid}\b")


class Pipeline(Env, unittest.TestCase):
    def test_a_job_with_a_pipeline_and_a_subshell_in_other_groups_is_stopped_and_continued_whole(self):
        """A stopper that signalled one group left the wrapper subshell and the `| tail -1` pipe stopped (w10's build, 67 min)."""
        job = f"(setsid bash -c 'while :; do echo x >> {self.count}; sleep 0.05; done') | tail -1"
        guard = self.start("bash", "-c", job)
        self.assertTrue(until(lambda: os.path.exists(self.count)))
        self.set_load(30.0, 30.0)
        root = int(subprocess.run(["pgrep", "-P", str(guard.pid), "-f", "tail -1"], capture_output=True, text=True).stdout.split()[0])
        alive = lambda: [p for p in [root, *descendants(root)] if state(p) not in ("gone", "Z")]
        self.assertTrue(until(lambda: len(alive()) >= 3 and all(state(p) == "T" for p in alive())), [(p, state(p)) for p in alive()])
        self.set_load(10.0, 10.0)
        self.assertTrue(until(lambda: not any(state(p) == "T" for p in alive())), [(p, state(p)) for p in alive()])


class Admission(Env, unittest.TestCase):
    def test_with_one_free_slot_only_one_of_two_concurrent_starters_runs(self):
        env = {"LG_SLOTS": "1", "LG_NO_WAIT": "1"}
        first = self.start("sleep", "3", **env)
        second = self.start("sleep", "3", **env)
        codes = sorted([first.wait(timeout=10), second.wait(timeout=10)])
        self.assertEqual(codes, [0, 75], "both starters ran, or neither")

    def test_a_waiting_starter_runs_when_the_slot_is_released(self):
        first = self.start("sleep", "30", LG_SLOTS="1")
        self.assertTrue(until(lambda: os.path.isdir(self.slots) and any(descendants(first.pid))))
        marker = os.path.join(self.dir.name, "ran")
        second = self.start("touch", marker, LG_SLOTS="1")
        time.sleep(0.5)
        self.assertFalse(os.path.exists(marker), "it ran without a slot")
        first.terminate()
        first.wait(timeout=5)
        self.assertTrue(until(lambda: os.path.exists(marker)), "the slot was not released")
        self.assertEqual(second.wait(timeout=5), 0)

    def test_no_job_starts_while_the_one_minute_load_is_high(self):
        self.set_load(30.0, 1.0)
        guard = self.start("true", LG_NO_WAIT="1")
        self.assertEqual(guard.wait(timeout=5), 75)
        self.assertIn("load too high", guard.communicate()[1])


    def test_a_queued_starter_says_so_at_once_and_when_admitted(self):
        """A starter waiting for load logged nothing: a session could not tell 'queued' from 'hung'."""
        self.set_load(30.0, 30.0)
        guard = self.start("true", LG_WAIT_LOG="0.3")
        time.sleep(1.0)
        self.assertIsNone(guard.poll(), "it started under load")
        self.set_load(1.0, 1.0)
        self.assertEqual(guard.wait(timeout=5), 0)
        err = guard.communicate()[1]
        waits = re.findall(r"load_guard: \d\d:\d\dZ waiting for load \(1m 30.0, 5m 30.0, slots 0/3 busy\)", err)
        self.assertGreaterEqual(len(waits), 2, err)
        self.assertRegex(err, r"load_guard: \d\d:\d\dZ admit slot \d \(1m 1.0\)")

    def test_a_starter_without_a_slot_says_it_waits_for_a_slot(self):
        first = self.start("sleep", "30", LG_SLOTS="1")
        self.assertTrue(until(lambda: os.path.isdir(self.slots) and any(descendants(first.pid))))
        second = self.start("true", LG_SLOTS="1", LG_WAIT_LOG="0.3")
        time.sleep(1.0)
        first.terminate()
        first.wait(timeout=5)
        self.assertEqual(second.wait(timeout=5), 0)
        self.assertRegex(second.communicate()[1], r"waiting for slot \(1m [\d.]+, 5m [\d.]+, slots 1/1 busy\)")


if __name__ == "__main__":
    unittest.main()
