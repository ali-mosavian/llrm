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
        self.meminfo = os.path.join(self.dir.name, "meminfo")
        self.set_mem(10)
        self.set_load(1.0, 1.0)
        self.guards = []

    def tearDown(self):
        for guard in self.guards:
            if guard.poll() is None:
                guard.terminate()
                guard.wait(timeout=5)
        self.dir.cleanup()

    def set_mem(self, gb):
        open(self.meminfo, "w").write(f"MemTotal: 200000000 kB\nMemAvailable: {gb * 1024 * 1024} kB\n")

    def set_load(self, one, five):
        open(self.load, "w").write(f"{one} {five} 1.0 1/1 1\n")

    def lines(self):
        return len(open(self.count).read().splitlines())

    def start(self, *command, **env):
        guard = subprocess.Popen(
            [GUARD, *command],
            env={**os.environ, "LG_LOAD_FILE": self.load, "LG_POLL": "0.1", "LG_SLOT_DIR": self.slots, "LG_MEMINFO": self.meminfo, **env},
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
        env = {"LG_SLOTS": "1", "LG_SLOTS_HIGH": "1", "LG_NO_WAIT": "1", "LG_SETTLE": "0"}
        first = self.start("sleep", "3", **env)
        second = self.start("sleep", "3", **env)
        codes = sorted([first.wait(timeout=10), second.wait(timeout=10)])
        self.assertEqual(codes, [0, 75], "both starters ran, or neither")

    def test_a_waiting_starter_runs_when_the_slot_is_released(self):
        first = self.start("sleep", "30", LG_SLOTS="1", LG_SLOTS_HIGH="1", LG_SETTLE="0")
        self.assertTrue(until(lambda: os.path.isdir(self.slots) and any(descendants(first.pid))))
        marker = os.path.join(self.dir.name, "ran")
        second = self.start("touch", marker, LG_SLOTS="1", LG_SLOTS_HIGH="1", LG_SETTLE="0")
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
        guard = self.start("true", LG_WAIT_LOG="0.3", LG_SETTLE="0")
        time.sleep(1.0)
        self.assertIsNone(guard.poll(), "it started under load")
        self.set_load(1.0, 1.0)
        self.assertEqual(guard.wait(timeout=5), 0)
        err = guard.communicate()[1]
        waits = re.findall(r"load_guard: \d\d:\d\dZ waiting for load \(1m 30.0, 5m 30.0, slots 0/3 busy\)", err)
        self.assertGreaterEqual(len(waits), 2, err)
        self.assertRegex(err, r"load_guard: \d\d:\d\dZ admit slot \d \(1m 1.0, slots 5\)")

    def test_a_starter_without_a_slot_says_it_waits_for_a_slot(self):
        first = self.start("sleep", "30", LG_SLOTS="1", LG_SLOTS_HIGH="1", LG_SETTLE="0")
        self.assertTrue(until(lambda: os.path.isdir(self.slots) and any(descendants(first.pid))))
        second = self.start("true", LG_SLOTS="1", LG_SLOTS_HIGH="1", LG_WAIT_LOG="0.3", LG_SETTLE="0")
        time.sleep(1.0)
        first.terminate()
        first.wait(timeout=5)
        self.assertEqual(second.wait(timeout=5), 0)
        self.assertRegex(second.communicate()[1], r"waiting for slot \(1m [\d.]+, 5m [\d.]+, slots 1/1 busy\)")


    def test_the_start_threshold_is_25_on_the_one_minute_load(self):
        self.set_load(24.9, 1.0)
        admitted = self.start("true", LG_NO_WAIT="1", LG_SETTLE="0")
        self.assertEqual(admitted.wait(timeout=5), 0)
        self.set_load(25.0, 1.0)
        refused = self.start("true", LG_NO_WAIT="1", LG_SETTLE="0")
        self.assertEqual(refused.wait(timeout=5), 75)

    def test_two_starters_are_admitted_at_least_a_settle_period_apart(self):
        """Both saw the same 1m load and both went; the load rose only after they had started."""
        env = {"LG_SETTLE": "1.5"}
        first = self.start("true", **env)
        self.assertEqual(first.wait(timeout=5), 0)
        began = time.time()
        second = self.start("true", **env)
        self.assertEqual(second.wait(timeout=10), 0)
        self.assertGreaterEqual(time.time() - began, 1.0, "the second starter did not wait for the first to settle")


class SlotCount(Env, unittest.TestCase):
    """Three slots were held by long jobs while the load sat at 12 and m68k starved; the count now follows the load."""

    def admitted(self, load, wanted):
        """How many of `wanted` long jobs the guard admits at 1m `load`."""
        self.set_load(load, load)
        jobs = [self.start("sleep", "30", LG_NO_WAIT="1", LG_SETTLE="0") for _ in range(wanted)]
        time.sleep(1.5)
        return sum(1 for job in jobs if job.poll() is None)

    def test_five_slots_while_the_one_minute_load_is_under_20(self):
        self.assertEqual(self.admitted(15.0, 7), 5)

    def test_three_slots_at_a_load_of_26_and_no_admission_at_all(self):
        self.assertEqual(self.admitted(23.0, 7), 3)  # 22..25: base count, admitting
        self.assertEqual(self.admitted(26.0, 1), 0, "a job was admitted at 1m 26")

    def test_between_20_and_22_the_count_stays_what_it_was(self):
        self.assertEqual(self.admitted(15.0, 1), 1)  # widens to 5
        self.set_load(21.0, 21.0)
        late = [self.start("sleep", "30", LG_NO_WAIT="1", LG_SETTLE="0") for _ in range(5)]
        time.sleep(1.5)
        self.assertEqual(sum(1 for job in late if job.poll() is None), 4, "21 must keep 5 slots (1 already running)")

    def test_a_fresh_start_at_21_has_three_slots(self):
        self.assertEqual(self.admitted(21.0, 5), 3)

    def test_quiet_jobs_are_admitted_only_under_a_load_of_20(self):
        self.set_load(21.0, 21.0)
        self.assertEqual(self.start("true", LG_QUIET="1", LG_NO_WAIT="1", LG_SETTLE="0").wait(timeout=5), 75)
        self.set_load(19.0, 19.0)
        self.assertEqual(self.start("true", LG_QUIET="1", LG_NO_WAIT="1", LG_SETTLE="0").wait(timeout=5), 0)

    def test_running_jobs_are_not_stopped_when_the_count_falls(self):
        self.assertEqual(self.admitted(15.0, 5), 5)
        self.set_load(24.0, 24.0)
        time.sleep(1.0)
        self.assertTrue(all(job.poll() is None for job in self.guards))


class Resume(Env, unittest.TestCase):
    """cargo, m68k and ct-rc-base sat stopped ~6 min at 1m 12-15 because the 5m average was still high."""

    def stopped_group(self):
        guard = self.start(*self.counter())
        self.assertTrue(until(lambda: os.path.exists(self.count)))
        pgid = int(subprocess.run(["pgrep", "-P", str(guard.pid), "-f", "while :"], capture_output=True, text=True).stdout.split()[0])
        self.set_load(30.0, 30.0)
        self.assertTrue(until(lambda: state(pgid) == "T"))
        return pgid

    def test_a_group_resumes_at_one_minute_12_though_five_minute_is_34(self):
        pgid = self.stopped_group()
        self.set_load(12.0, 34.0)
        self.assertTrue(until(lambda: state(pgid) != "T"), "stayed stopped at 1m 12 / 5m 34")

    def test_a_group_stays_stopped_at_one_minute_22_and_five_minute_34(self):
        pgid = self.stopped_group()
        self.set_load(22.0, 34.0)
        time.sleep(1.0)
        self.assertEqual(state(pgid), "T")


if __name__ == "__main__":
    unittest.main()


class Order(Env, unittest.TestCase):
    """27 waiters drained at one per minute in no order: a miscompile build waited behind benches."""

    def admit_order(self, tiers, **env):
        self.set_load(30.0, 30.0)  # nobody starts until all are queued
        log = os.path.join(self.dir.name, "order")
        jobs = []
        for tier in tiers:
            jobs.append(self.start("bash", "-c", f"echo {tier} >> {log}", LG_TIER=str(tier), LG_SETTLE="0", LG_POLL="0.05", **env))
            time.sleep(0.3)
        self.set_load(1.0, 1.0)
        for job in jobs:
            job.wait(timeout=15)
        return open(log).read().split()

    def test_waiters_are_admitted_by_tier_not_by_arrival(self):
        self.assertEqual(self.admit_order([3, 1, 2]), ["1", "2", "3"])

    def test_inside_a_tier_the_earlier_waiter_goes_first(self):
        self.assertEqual(self.admit_order([2, 2, 2]), ["2", "2", "2"])

    def test_four_jobs_at_a_low_load_start_within_the_fast_settle(self):
        self.set_load(5.0, 5.0)
        began = time.time()
        jobs = [self.start("true", LG_SETTLE="60", LG_SETTLE_FAST="1", LG_SLOTS="5", LG_POLL="0.05") for _ in range(4)]
        for job in jobs:
            self.assertEqual(job.wait(timeout=15), 0)
        self.assertLess(time.time() - began, 8, "the 60 s settle applied at 1m load 5")


class Drain(Env, unittest.TestCase):
    """The 15 s settle applied only under 1m 15, but each admission lifts the load past 15: the queue of 27 drained at one per minute."""

    def test_four_jobs_at_one_minute_16_start_within_the_fast_settle(self):
        self.set_load(16.0, 16.0)
        began = time.time()
        jobs = [self.start("true", LG_SETTLE="60", LG_SETTLE_FAST="1", LG_POLL="0.05") for _ in range(4)]
        for job in jobs:
            self.assertEqual(job.wait(timeout=15), 0)
        self.assertLess(time.time() - began, 10, "the 60 s settle applied at 1m load 16")

    def test_the_fast_settle_does_not_apply_at_one_minute_23(self):
        self.set_load(23.0, 23.0)
        first = self.start("true", LG_SETTLE="60", LG_SETTLE_FAST="1", LG_POLL="0.05")
        self.assertEqual(first.wait(timeout=5), 0)
        second = self.start("true", LG_SETTLE="60", LG_SETTLE_FAST="1", LG_POLL="0.05")
        time.sleep(3)
        self.assertIsNone(second.poll(), "second starter ran inside the 60 s settle at 1m 23")

    def admitted(self, load, gb, wanted=9):
        self.set_load(load, load)
        self.set_mem(gb)
        jobs = [self.start("sleep", "30", LG_NO_WAIT="1", LG_SETTLE="0") for _ in range(wanted)]
        time.sleep(2)
        return sum(1 for job in jobs if job.poll() is None)

    def test_seven_slots_at_one_minute_17_with_80_gb_available(self):
        self.assertEqual(self.admitted(17.0, 80), 7)

    def test_five_slots_at_one_minute_17_with_30_gb_available(self):
        self.assertEqual(self.admitted(17.0, 30), 5)

    def test_five_slots_at_one_minute_19_with_80_gb_available(self):
        self.assertEqual(self.admitted(19.0, 80), 5)

    def test_tier_order_holds_with_the_wider_count(self):
        self.set_load(30.0, 30.0)
        log = os.path.join(self.dir.name, "order")
        jobs = []
        for tier in (3, 1, 2):
            jobs.append(self.start("bash", "-c", f"echo {tier} >> {log}", LG_TIER=str(tier), LG_SETTLE="0", LG_POLL="0.05"))
            time.sleep(0.3)
        self.set_load(16.0, 16.0)
        for job in jobs:
            job.wait(timeout=15)
        self.assertEqual(open(log).read().split(), ["1", "2", "3"])
