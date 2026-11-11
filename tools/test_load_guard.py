"""load_guard.sh admits a bounded number of jobs, stops a job's whole process tree at high load, resumes exactly it, and never leaves it stopped."""

import os
import re
import signal
import subprocess
import tempfile
import threading
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


# The mechanism tests run on the thresholds they were written for; the shipped defaults are asserted once, in Defaults.
PROFILE = {
    "LG_STOP_5M": "28", "LG_STOP_1M": "38", "LG_RESUME_5M": "26", "LG_RESUME_1M": "20", "LG_START_1M": "25", "LG_SLOTS": "3", "LG_SLOTS_HIGH": "5",
    "LG_SLOTS_MAX": "7", "LG_SLOTS_WIDEST": "10", "LG_WIDEST_1M": "17", "LG_WIDEN_1M": "20", "LG_NARROW_1M": "22", "LG_SETTLE": "60", "LG_SETTLE_FAST": "15",
    "LG_FAST_1M": "22", "LG_WIDE_1M": "22", "LG_AGE_STEP": "1200", "LG_TIER": "4",
}


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
        self.leaked = []  # pids of a job's other groups: a TERM to the wrapper reaches only the job's own group

    def tearDown(self):
        for guard in self.guards:
            if guard.poll() is None:
                guard.terminate()
                guard.wait(timeout=5)
        for pid in self.leaked:  # a loop left writing into self.dir made its removal fail once in 6 runs: "Directory not empty"
            try:
                os.kill(pid, signal.SIGKILL)
            except OSError:
                pass
        time.sleep(0.1)
        self.dir.cleanup()

    def queued(self, job, seconds=60):
        """Wait until the starter's line is in the prio file: ordering tests that sleep a fixed time between starts flip under host load 40 when a starter is slow to start."""
        prio = os.path.join(self.slots, "prio")
        def listed():
            try:
                return any(line.split()[2:3] == [str(job.pid)] for line in open(prio))
            except OSError:
                return False
        self.assertTrue(until(listed, seconds), f"{job.pid} never queued")

    def decided(self, jobs, seconds=60):
        """Wait until every starter has refused (exited) or been admitted (its job record exists): serialized admission under host load 60 left a fixed 1.5 s window counting starters still queued as admitted (11 of 12 'running' for a cap of 5)."""
        records = os.path.join(self.slots, "jobs")
        until(lambda: all(job.poll() is not None or os.path.exists(os.path.join(records, str(job.pid))) for job in jobs), seconds)

    def set_mem(self, gb):
        open(self.meminfo, "w").write(f"MemTotal: 200000000 kB\nMemAvailable: {gb * 1024 * 1024} kB\n")

    def set_load(self, one, five):
        open(self.load, "w").write(f"{one} {five} 1.0 1/1 1\n")

    def lines(self):
        return len(open(self.count).read().splitlines())

    def start(self, *command, **env):
        guard = subprocess.Popen(
            [GUARD, *command],
            env={**os.environ, "LG_LOAD_FILE": self.load, "LG_POLL": "0.1", "LG_SLOT_DIR": self.slots, "LG_MEMINFO": self.meminfo, **PROFILE, **env},
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
        seen = []  # the group is continued before the guard logs the resume: read stderr until both lines are in, then terminate
        reader = threading.Thread(target=lambda: [seen.append(line) for line in self.guard.stderr])
        reader.start()
        self.assertTrue(until(lambda: "resume groups" in "".join(seen), 20), "".join(seen))
        self.guard.terminate()
        reader.join(timeout=5)
        err = "".join(seen)
        for word in ("stop", "resume"):
            self.assertRegex(err, rf"load_guard: \d\d:\d\dZ {word} groups .*\b{pgid}\b")


class Teardown(Env, unittest.TestCase):
    def test_the_directory_is_removed_though_a_leaked_group_keeps_writing_into_it(self):
        """The 1-in-6 error of the suite: a job's setsid'd loop outlived the TERM to its wrapper and refilled the temp dir while it was being removed ("Directory not empty")."""
        loop = subprocess.Popen(["bash", "-c", f"while :; do mkdir -p {self.dir.name}/x; echo > {self.dir.name}/x/$RANDOM; done"], start_new_session=True)
        self.leaked.append(loop.pid)
        time.sleep(0.2)
        self.tearDown()
        self.assertFalse(os.path.exists(self.dir.name))
        loop.wait(timeout=5)


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
        self.leaked.extend(alive())
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
        seen = []  # wait for two "waiting" lines before the load drops: start-up under host load 40 took over the fixed 1 s
        reader = threading.Thread(target=lambda: [seen.append(line) for line in guard.stderr])
        reader.start()
        self.assertTrue(until(lambda: "".join(seen).count("waiting for load") >= 2, 30), "".join(seen))
        self.assertIsNone(guard.poll(), "it started under load")
        self.set_load(1.0, 1.0)
        self.assertEqual(guard.wait(timeout=15), 0)
        reader.join(timeout=5)
        err = "".join(seen)
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
        self.assertEqual(first.wait(timeout=20), 0)
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
        self.decided(jobs)
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
        self.decided(late)
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


class SlotFile(Env, unittest.TestCase):
    """A waiter's `echo > slots_now` truncates the file; another waiter's `cat` then read an empty count, `[ $busy -lt $slots ]` failed
    with "integer expression expected", and that pass admitted nobody (13:39Z, every waiter's log)."""

    def test_slot_count_in_the_mid_band_never_prints_empty_beside_a_truncating_writer(self):
        os.makedirs(self.slots)
        slot_file = os.path.join(self.slots, "slots_now")
        open(slot_file, "w").write("3\n")
        script = open(GUARD).read()
        start = script.index("slot_count() {")
        body = script[start:script.index("\n}\n", start) + 3]
        harness = (
            "above() { awk -v a=\"$1\" -v b=\"$2\" 'BEGIN{exit !(a>b)}'; }\n"
            f"base=3 high=5 widen1=20 narrow1=22 wide=7 widest=10 widest1=14 slot_dir={self.slots}\n{body}\n"
            f"( while :; do : > {slot_file}; echo 3 > {slot_file}; done ) & w=$!\n"
            "empty=0; for i in $(seq 1 400); do n=$(slot_count 21 2>/dev/null | tail -1); [ -n \"$n\" ] || empty=$((empty+1)); done\n"
            "kill $w; echo empty=$empty\n"
        )
        done = subprocess.run(["bash", "-c", harness], capture_output=True, text=True, timeout=60)
        self.assertEqual(done.stdout.strip().splitlines()[-1], "empty=0", done.stdout + done.stderr)


class TermInsideTheQueue(Env, unittest.TestCase):
    """A waiter that got SIGTERM inside prio_edit ran its EXIT trap (prio_edit del) in the same shell, which flocked prio.lock a
    second time on a new descriptor and waited for itself: the waiter never died (test_the_fast_settle... timed out in tearDown under load)."""

    def test_a_waiter_terminated_while_it_holds_the_priority_lock_exits(self):
        wrapper_dir = os.path.join(self.dir.name, "bin")
        os.makedirs(wrapper_dir)
        slow = os.path.join(wrapper_dir, "flock")
        open(slow, "w").write("#!/bin/sh\n/usr/bin/flock \"$@\"; rc=$?; sleep 0.3; exit $rc\n")
        os.chmod(slow, 0o755)
        self.set_load(23.0, 23.0)
        env = {"LG_SETTLE": "60", "LG_SETTLE_FAST": "1"}
        self.assertEqual(self.start("true", **env).wait(timeout=10), 0)  # leaves the settle period running
        path = f"{wrapper_dir}:{os.environ['PATH']}"
        for attempt in range(6):  # a waiter spends most of each poll inside prio_edit first, holding the lock
            guard = self.start("true", PATH=path, **env)
            time.sleep(0.4 + 0.07 * attempt)
            guard.terminate()
            self.assertTrue(until(lambda: guard.poll() is not None, 3), f"attempt {attempt}: stuck on its own lock")  # LG_LOCK_WAIT 2 s
            self.assertTrue(until(lambda: self.slots_free(), 2), f"attempt {attempt}: a slot stayed locked after the exit")

    def slots_free(self):
        import fcntl
        slot_dir = os.path.join(self.dir.name, "slots")
        for name in os.listdir(slot_dir):
            if not name.startswith("slot."):
                continue
            with open(os.path.join(slot_dir, name)) as f:
                try:
                    fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
                except OSError:
                    return False
        return True


class Cancel(Env, unittest.TestCase):
    """--cancel JOBID kills exactly the recorded process group. Pattern kills (pgrep -f / pkill -f) took out three sessions' builds on 2026-11-10."""

    def job(self, *command):
        guard = self.start(*command, LG_SLOTS="5", LG_SLOTS_HIGH="5", LG_SETTLE="0")
        record = os.path.join(self.slots, "jobs", str(guard.pid))
        self.assertTrue(until(lambda: os.path.exists(record)), "no job record written")
        return guard

    def cancel(self, ident):
        return subprocess.run([GUARD, "--cancel", str(ident)], env={**os.environ, "LG_SLOT_DIR": self.slots}, capture_output=True, text=True)

    def test_cancel_kills_the_recorded_job_and_leaves_one_with_the_same_command_line(self):
        first, second = self.job("sleep", "300"), self.job("sleep", "300")
        done = self.cancel(first.pid)
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn("cancelled job", done.stdout)
        self.assertTrue(until(lambda: first.poll() is not None), "the cancelled job kept running")
        self.assertIsNone(second.poll(), "the job with the same command line was killed")
        self.assertTrue(any(descendants(second.pid)), "the second job's command is gone")
        self.assertFalse(os.path.exists(os.path.join(self.slots, "jobs", str(first.pid))), "the record outlived the job")

    def test_an_unknown_id_exits_nonzero_and_kills_nothing(self):
        job = self.job("sleep", "300")
        done = self.cancel(999999)
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("no recorded job", done.stderr)
        self.assertIsNone(job.poll())

    def test_a_finished_job_is_refused(self):
        job = self.job("sleep", "300")
        record = os.path.join(self.slots, "jobs", str(job.pid))
        saved = open(record).read()
        job.terminate()
        job.wait(timeout=5)
        open(record, "w").write(saved)  # a record left behind by a wrapper that was SIGKILLed
        self.assertNotEqual(self.cancel(job.pid).returncode, 0)
        self.assertFalse(os.path.exists(record), "the stale record was kept")

    def test_a_record_naming_a_pgid_that_is_not_the_wrappers_child_is_refused(self):
        bystander = subprocess.Popen(["sleep", "300"], start_new_session=True)
        holder = subprocess.Popen(["sleep", "300"])
        try:
            os.makedirs(os.path.join(self.slots, "jobs"), exist_ok=True)
            open(os.path.join(self.slots, "jobs", "4242"), "w").write(f"{holder.pid} {bystander.pid} forged\n")
            self.assertNotEqual(self.cancel(4242).returncode, 0)
            self.assertIsNone(bystander.poll(), "a forged record killed an unrecorded group")
        finally:
            bystander.kill(); holder.kill(); bystander.wait(); holder.wait()


HOLDER = "import fcntl, os, sys, time\nfd = os.open(sys.argv[1], os.O_RDWR | os.O_CREAT)\nfcntl.flock(fd, fcntl.LOCK_EX)\nos.write(fd, (str(os.getpid()) + chr(10)).encode())\nif len(sys.argv) > 2:\n    fd2 = os.open(sys.argv[2], os.O_RDWR | os.O_CREAT)\n    fcntl.flock(fd2, fcntl.LOCK_EX)\n    os.write(fd2, (str(os.getpid()) + chr(10)).encode())\nprint('held', flush=True)\ntime.sleep(300)\n"


class ActiveHolders(Env, unittest.TestCase):
    """Admission counted locked slot files. A stopped cargo test held slot.3 for 28.8 h; a pid holding two files read as two jobs."""

    def holder(self, *slots):
        os.makedirs(self.slots, exist_ok=True)
        proc = subprocess.Popen(["python3", "-c", HOLDER, *[os.path.join(self.slots, f"slot.{n}") for n in slots]], stdout=subprocess.PIPE, text=True)
        self.assertEqual(proc.stdout.readline().strip(), "held")
        self.addCleanup(lambda: (proc.kill(), proc.wait()))
        return proc

    def admitted(self, slots):
        env = {"LG_SLOTS": str(slots), "LG_SLOTS_HIGH": str(slots), "LG_SLOTS_MAX": str(slots), "LG_SLOTS_WIDEST": "6", "LG_NO_WAIT": "1", "LG_SETTLE": "0"}
        return self.start("true", **env).wait(timeout=10) == 0

    def test_a_running_holder_takes_the_only_slot(self):
        self.holder(1)
        self.assertFalse(self.admitted(1))

    def test_a_stopped_holder_does_not_take_a_slot(self):
        holder = self.holder(1)
        os.kill(holder.pid, signal.SIGSTOP)
        self.assertTrue(until(lambda: state(holder.pid).startswith("T")))
        self.assertTrue(self.admitted(1), "a stopped holder blocked admission")

    def test_one_pid_holding_two_slot_files_counts_once(self):
        self.holder(1, 2)
        self.assertTrue(self.admitted(2), "one job on two slot files ate two slots")
        self.assertFalse(self.admitted(1))

    def slot_held(self, n):
        import fcntl

        try:
            fd = os.open(os.path.join(self.slots, f"slot.{n}"), os.O_RDWR)
        except OSError:
            return False
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            return False
        except OSError:
            return True
        finally:
            os.close(fd)

    def test_two_starters_deciding_at_once_do_not_both_take_a_slot_under_a_cap_of_one(self):
        """The loop's last flake (test_a_starter_without_a_slot_says_it_waits_for_a_slot, 2 in 25 at load 60): the first starter holds slot 1 while it decides, the second counted
        it as a probe, took slot 2 and went, then the first went too. Counting, taking and recording are one critical section."""
        first = self.start("sleep", "30", LG_SLOTS="1", LG_SLOTS_HIGH="1", LG_SETTLE="0", LG_TEST_PROBE_PAUSE="8")
        self.assertTrue(until(lambda: self.slot_held(1), 30), "the first starter never took slot 1")  # it holds slot.1, undecided, for 8 s
        second = self.start("true", LG_SLOTS="1", LG_SLOTS_HIGH="1", LG_NO_WAIT="1", LG_SETTLE="0", LG_SCAN_EVERY="0")
        self.assertEqual(second.wait(timeout=30), 75, "two starters were admitted under a cap of one")

    def test_a_job_admitted_but_not_yet_spawned_already_counts(self):
        """The 1-in-6 failure of the loop: a starter right behind an admitted one found it holding the slot with no record and no child yet, read it as a probe, and took slot 2 (slots 1)."""
        first = self.start("sleep", "30", LG_SLOTS="1", LG_SLOTS_HIGH="1", LG_SETTLE="0", LG_TEST_SPAWN_PAUSE="3")
        self.assertTrue(until(lambda: os.path.isdir(os.path.join(self.slots, "jobs")) and os.listdir(os.path.join(self.slots, "jobs"))))
        record = os.path.join(self.slots, "jobs", str(first.pid))
        self.assertEqual(open(record).read().split()[1], "0", "the first job was already spawned: the pause did not hold")
        second = self.start("true", LG_SLOTS="1", LG_SLOTS_HIGH="1", LG_NO_WAIT="1", LG_SETTLE="0", LG_SCAN_EVERY="0")
        self.assertEqual(second.wait(timeout=10), 75, "a job in the window between admission and spawn did not count")

    def test_twelve_starters_arriving_together_are_all_admitted_under_a_cap_of_twelve(self):
        """The 1-in-4 flake of an earlier scan: starters read each other's probe lock as running jobs and all refused (0 of 12 admitted). Probes now run under admit.lock."""
        env = {"LG_SLOTS": "12", "LG_SLOTS_HIGH": "12", "LG_SLOTS_MAX": "12", "LG_SLOTS_WIDEST": "12", "LG_NO_WAIT": "1", "LG_SETTLE": "0"}
        jobs = [self.start("sleep", "30", **env) for _ in range(12)]
        self.decided(jobs)
        self.assertEqual(sum(1 for job in jobs if job.poll() is None), 12)


class NoProcWalk(Env, unittest.TestCase):
    """The holder scan was `find /proc -maxdepth 3 ...` on every poll of every waiter: about 106% of a core at load 70, so the guard was part of the load it limits.
    The holder's pid is now written into its slot file, so no process table is walked."""

    def shim(self):
        """A `find` and a `ps` that log their use; PATH puts them first."""
        bindir = os.path.join(self.dir.name, "bin")
        os.makedirs(bindir)
        self.calls = os.path.join(self.dir.name, "calls")
        for tool in ("find",):
            real = subprocess.check_output(["which", tool], text=True).strip()
            path = os.path.join(bindir, tool)
            open(path, "w").write(f'#!/bin/sh\necho "{tool} $*" >> {self.calls}\nexec {real} "$@"\n')
            os.chmod(path, 0o755)
        return {"PATH": bindir + os.pathsep + os.environ["PATH"]}

    def test_admission_and_waiting_never_run_find(self):
        env = self.shim()
        first = self.start("sleep", "30", LG_SLOTS="1", LG_SLOTS_HIGH="1", LG_SETTLE="0", **env)
        self.decided([first])
        waiter = self.start("true", LG_SLOTS="1", LG_SLOTS_HIGH="1", LG_SETTLE="0", LG_WAIT_LOG="0.2", **env)
        self.queued(waiter)
        time.sleep(1.5)
        self.assertFalse(os.path.exists(self.calls) and open(self.calls).read(), "the guard walked /proc with find")

    def test_the_script_has_no_proc_fd_walk(self):
        self.assertNotIn("-path '/proc/", open(GUARD).read())

    def test_one_more_starter_than_slots_is_refused_and_a_dead_holder_is_reclaimed(self):
        env = {"LG_SLOTS": "2", "LG_SLOTS_HIGH": "2", "LG_SLOTS_MAX": "2", "LG_SETTLE": "0", "LG_NO_WAIT": "1"}
        jobs = [self.start("sleep", "30", **env) for _ in range(2)]
        self.decided(jobs)
        self.assertEqual(self.start("true", **env).wait(timeout=10), 75, "a third job was admitted under a cap of 2")
        jobs[0].kill()
        jobs[0].wait()
        self.assertTrue(until(lambda: self.start("true", **env).wait(timeout=10) == 0, 15), "a killed holder's slot was never reclaimed")

    def test_a_slot_file_naming_a_dead_pid_is_free_when_unlocked(self):
        os.makedirs(self.slots, exist_ok=True)
        open(os.path.join(self.slots, "slot.1"), "w").write("999999\n")
        env = {"LG_SLOTS": "1", "LG_SLOTS_HIGH": "1", "LG_SLOTS_MAX": "1", "LG_SETTLE": "0", "LG_NO_WAIT": "1"}
        self.assertEqual(self.start("true", **env).wait(timeout=10), 0)


class Defaults(unittest.TestCase):
    """The shipped thresholds (oversight 11-10): start 48, stop 5m 50 / 1m 60, resume 5m 44 / 1m 40, 20 slots (24 widest). The repo copy had drifted to the pre-11-08 numbers."""

    def test_the_default_thresholds_and_slot_counts(self):
        text = open(GUARD).read()
        for name, value in (("LG_START_1M", 48), ("LG_STOP_5M", 50), ("LG_STOP_1M", 60), ("LG_RESUME_5M", 44), ("LG_RESUME_1M", 40), ("LG_SLOTS", 20), ("LG_SLOTS_MAX", 20), ("LG_SLOTS_WIDEST", 24)):
            self.assertIn("${%s:-%d}" % (name, value), text, name)

    def test_a_job_starts_at_load_47_and_not_at_49_with_no_env(self):
        for load, code in ((47.0, 0), (49.0, 75)):
            with tempfile.TemporaryDirectory() as d:
                open(os.path.join(d, "load"), "w").write(f"{load} 1.0 1.0 1/1 1\n")
                env = {k: v for k, v in os.environ.items() if not k.startswith("LG_")}
                env.update(LG_LOAD_FILE=os.path.join(d, "load"), LG_SLOT_DIR=os.path.join(d, "slots"), LG_NO_WAIT="1", LG_SETTLE="0", LG_TIER="4")
                self.assertEqual(subprocess.run([GUARD, "true"], env=env, capture_output=True).returncode, code, load)


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
            self.queued(jobs[-1])
        self.set_load(1.0, 1.0)
        for job in jobs:
            job.wait(timeout=15)
        return open(log).read().split()

    def test_tiers_one_to_three_are_admitted_by_age_and_tier_four_after_them(self):
        """Oversight 11-10: age priority across tiers 1-3; the tier breaks ties only. Tier 4 stays behind all of them."""
        self.assertEqual(self.admit_order([3, 1, 2]), ["3", "1", "2"])

    def test_tier_four_waits_behind_older_tiers_one_to_three(self):
        self.assertEqual(self.admit_order([4, 3, 1]), ["3", "1", "4"])

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
        self.assertEqual(first.wait(timeout=20), 0)
        second = self.start("true", LG_SETTLE="60", LG_SETTLE_FAST="1", LG_POLL="0.05")
        time.sleep(3)
        self.assertIsNone(second.poll(), "second starter ran inside the 60 s settle at 1m 23")

    def admitted(self, load, gb, wanted=9):
        self.set_load(load, load)
        self.set_mem(gb)
        jobs = [self.start("sleep", "30", LG_NO_WAIT="1", LG_SETTLE="0") for _ in range(wanted)]
        self.decided(jobs)
        return sum(1 for job in jobs if job.poll() is None)

    def test_seven_slots_at_one_minute_17_with_80_gb_available(self):
        self.assertEqual(self.admitted(17.0, 80), 7)

    def test_five_slots_at_one_minute_17_with_30_gb_available(self):
        self.assertEqual(self.admitted(17.0, 30), 5)

    def test_seven_slots_at_one_minute_21_with_80_gb_available(self):
        """Five slots at 1m 18.5 left 34 waiters queued with load 18 and the start limit 25: slots, not load, were the bottleneck."""
        self.assertEqual(self.admitted(21.0, 80), 7)

    def test_three_slots_at_one_minute_23_with_80_gb_available(self):
        self.assertEqual(self.admitted(23.0, 80), 3)

    def test_a_waiter_wakes_within_ten_seconds_of_the_load_dropping(self):
        """The 15 s default poll let only the head waiter admit every 15-30 s: 38 queued at 1m 8, five slots free."""
        self.set_load(30.0, 30.0)
        waiter = self.start("true", LG_POLL="", LG_SETTLE="0")
        time.sleep(1)
        self.set_load(1.0, 1.0)
        self.assertEqual(waiter.wait(timeout=13), 0, "the waiter slept through the 15 s default poll")  # 13 s: under host load 40 the default 5 s poll plus start-up passed 10 s once in 8

    def test_an_aged_tier_3_waiter_outranks_a_fresh_tier_2(self):
        """Tier 2 arrivals at 3-4 per minute starved every tier-3 PR gate for over an hour."""
        self.set_load(30.0, 30.0)
        log = os.path.join(self.dir.name, "order")
        jobs = [self.start("bash", "-c", f"echo 3 >> {log}", LG_TIER="3", LG_SETTLE="0", LG_POLL="0.05", LG_AGE_STEP="2")]
        time.sleep(3)
        jobs.append(self.start("bash", "-c", f"echo 2 >> {log}", LG_TIER="2", LG_SETTLE="0", LG_POLL="0.05", LG_AGE_STEP="2"))
        time.sleep(0.5)
        self.set_load(1.0, 1.0)
        for job in jobs:
            job.wait(timeout=15)
        self.assertEqual(open(log).read().split(), ["3", "2"])

    def test_an_old_tier_2_waiter_goes_before_fresh_tier_1_waiters(self):
        """A tier-2 build (6502-a) waited 2 h 20 min behind a stream of fresh tier-1 probes: tier 2 never aged to tier 1. Tiers 1-3 go by age; the tier breaks ties."""
        self.set_load(30.0, 30.0)
        log = os.path.join(self.dir.name, "order")
        jobs = [self.start("bash", "-c", f"echo 2 >> {log}", LG_TIER="2", LG_SETTLE="0", LG_POLL="0.05")]
        self.queued(jobs[0])
        time.sleep(1.5)
        for _ in range(3):
            jobs.append(self.start("bash", "-c", f"echo 1 >> {log}", LG_TIER="1", LG_SETTLE="0", LG_POLL="0.05"))
            self.queued(jobs[-1])
        self.set_load(1.0, 1.0)
        for job in jobs:
            job.wait(timeout=15)
        self.assertEqual(open(log).read().split()[0], "2")

    def seed_prio(self, tier, stamp, pid):
        os.makedirs(self.slots, exist_ok=True)
        with open(os.path.join(self.slots, "prio"), "a") as prio:
            prio.write(f"{tier} {stamp} {pid} 48\n")

    def test_a_fresh_tier_1_waiter_is_not_admitted_while_an_older_tier_2_entry_of_another_copy_is_alive(self):
        """The prio file is shared by every copy of the script alive at once (a running bash keeps its old inode after a deploy). An entry written by another copy, here a live
        old tier-2 one, ranks by its stamp: the new copy's tier-1 waiter must wait for it."""
        self.set_load(1.0, 1.0)
        older = subprocess.Popen(["sleep", "300"])
        self.addCleanup(older.kill)
        self.seed_prio(2, time.time() - 4200, older.pid)
        marker = os.path.join(self.dir.name, "ran")
        job = self.start("touch", marker, LG_TIER="1", LG_SETTLE="0", LG_POLL="0.05")
        time.sleep(1.5)
        self.assertFalse(os.path.exists(marker), "a fresh tier-1 job went before a 70-minute-old tier-2 entry")
        older.kill()
        older.wait()
        self.assertEqual(job.wait(timeout=10), 0)

    def test_an_older_waiter_that_does_not_fit_the_free_slots_still_holds_the_line_for_a_younger_one_that_does(self):
        """Reading 1 of the 07:14Z admission: the slot class does not reorder. Each waiter decides `busy < its own LG_SLOTS`, but `first` ranks by stamp only, so a younger
        job with a larger LG_SLOTS waits behind an older one that cannot fit (head-of-line blocking, by design: it keeps the old job from starving)."""
        self.set_load(1.0, 1.0)
        holder = self.start("sleep", "300", LG_SLOTS="1", LG_SLOTS_HIGH="1", LG_SLOTS_MAX="1", LG_SETTLE="0")
        self.assertTrue(until(lambda: os.path.isdir(os.path.join(self.slots, "jobs")) and os.listdir(os.path.join(self.slots, "jobs"))))
        older = self.start("true", LG_TIER="2", LG_SLOTS="1", LG_SLOTS_HIGH="1", LG_SLOTS_MAX="1", LG_SETTLE="0", LG_POLL="0.05")
        time.sleep(0.5)
        marker = os.path.join(self.dir.name, "ran")
        younger = self.start("touch", marker, LG_TIER="1", LG_SLOTS="5", LG_SLOTS_HIGH="5", LG_SLOTS_MAX="5", LG_SETTLE="0", LG_POLL="0.05")
        time.sleep(1.5)
        self.assertFalse(os.path.exists(marker), "the younger job passed the older one because it fits")
        self.assertIsNone(older.poll())

    def test_an_older_entry_whose_own_start_threshold_is_not_met_does_not_hold_the_line(self):
        """Reading 2: `first` skips an entry whose start threshold is at or under the 1m load (a quiet job needs < 20) or whose pid is dead; all other entries rank by stamp."""
        self.set_load(22.0, 22.0)
        quiet = subprocess.Popen(["sleep", "300"])
        self.addCleanup(quiet.kill)
        os.makedirs(self.slots, exist_ok=True)
        open(os.path.join(self.slots, "prio"), "w").write(f"3 {time.time() - 4200} {quiet.pid} 20\n")
        marker = os.path.join(self.dir.name, "ran")
        job = self.start("touch", marker, LG_TIER="1", LG_SETTLE="0", LG_POLL="0.05")
        self.assertEqual(job.wait(timeout=10), 0)
        self.assertTrue(os.path.exists(marker))

    def test_a_resubmitted_job_keeps_its_place_with_LG_QUEUED_AT(self):
        """--cancel and resubmit lost the queue place (a 2 h 20 min wait restarted at zero)."""
        self.set_load(30.0, 30.0)
        log = os.path.join(self.dir.name, "order")
        first = self.start("bash", "-c", f"echo fresh >> {log}", LG_TIER="2", LG_SETTLE="0", LG_POLL="0.05")
        time.sleep(0.5)
        resubmitted = self.start("bash", "-c", f"echo resubmitted >> {log}", LG_TIER="2", LG_SETTLE="0", LG_POLL="0.05", LG_QUEUED_AT=str(time.time() - 4200))
        time.sleep(0.5)
        self.set_load(1.0, 1.0)
        for job in (first, resubmitted):
            job.wait(timeout=15)
        self.assertEqual(open(log).read().split(), ["resubmitted", "fresh"])

    def test_a_queued_at_in_the_future_or_not_a_number_is_the_time_now(self):
        self.set_load(30.0, 30.0)
        for bad in (str(time.time() + 99999), "abc"):
            job = self.start("true", LG_TIER="2", LG_SETTLE="0", LG_POLL="0.05", LG_QUEUED_AT=bad)
            self.assertTrue(until(lambda: os.path.exists(os.path.join(self.slots, "prio")) and str(job.pid) in open(os.path.join(self.slots, "prio")).read()))
            stamp = [line.split()[1] for line in open(os.path.join(self.slots, "prio")) if line.split()[2] == str(job.pid)][0]
            self.assertLess(abs(float(stamp) - time.time()), 5, bad)
            job.terminate()
            job.wait(timeout=5)

    def test_an_aged_tier_4_waiter_never_passes_a_tier_2(self):
        self.set_load(30.0, 30.0)
        log = os.path.join(self.dir.name, "order")
        jobs = [self.start("bash", "-c", f"echo 4 >> {log}", LG_TIER="4", LG_SETTLE="0", LG_POLL="0.05", LG_AGE_STEP="1")]
        time.sleep(4)
        jobs.append(self.start("bash", "-c", f"echo 2 >> {log}", LG_TIER="2", LG_SETTLE="0", LG_POLL="0.05", LG_AGE_STEP="1"))
        time.sleep(0.5)
        self.set_load(1.0, 1.0)
        for job in jobs:
            job.wait(timeout=15)
        self.assertEqual(open(log).read().split(), ["2", "4"])

    def test_a_waiter_whose_prio_line_was_erased_is_still_admitted(self):
        """An old-copy waiter's prune read 'tier start pid 25' as pid '<pid> 25' and dropped every new line: new waiters never became first and starved with slots free."""
        self.set_load(30.0, 30.0)
        waiter = self.start("true", LG_TIER="3", LG_SETTLE="0", LG_POLL="0.05")
        time.sleep(0.5)
        open(os.path.join(self.slots, "prio"), "w").close()
        self.set_load(1.0, 1.0)
        self.assertEqual(waiter.wait(timeout=10), 0, "the waiter never became first after its line was erased")

    def test_ten_slots_at_one_minute_13_with_80_gb_available(self):
        self.assertEqual(self.admitted(13.0, 80, wanted=12), 10)

    def test_ten_slots_at_one_minute_16_with_80_gb_available(self):
        """Seven slots at 1m 16 left 38 waiters queued with load 16 and the start limit 25."""
        self.assertEqual(self.admitted(16.0, 80, wanted=12), 10)

    def test_seven_slots_at_one_minute_18_with_80_gb_available(self):
        self.assertEqual(self.admitted(18.0, 80, wanted=12), 7)

    def test_five_slots_at_one_minute_13_with_30_gb_available(self):
        self.assertEqual(self.admitted(13.0, 30, wanted=12), 5)

    def test_slots_wide_file_holds_the_wide_count(self):
        self.admitted(13.0, 80, wanted=1)
        self.assertEqual(open(os.path.join(self.slots, "slots_wide")).read().strip(), "10")

    def test_a_quiet_head_does_not_block_a_normal_waiter_at_one_minute_22(self):
        """A LG_QUIET tier-1 waiter (start line 20) at the head of the queue starved every normal waiter in the 20-25 band."""
        self.set_load(22.0, 22.0)
        quiet = self.start("sleep", "30", LG_TIER="1", LG_QUIET="1", LG_SETTLE="0")
        time.sleep(0.5)
        normal = self.start("true", LG_TIER="4", LG_SETTLE="0")
        self.assertEqual(normal.wait(timeout=5), 0, "the quiet head blocked a waiter whose start line is 25")
        self.assertIsNone(quiet.poll())

    def test_tier_order_holds_with_the_wider_count(self):
        self.set_load(30.0, 30.0)
        log = os.path.join(self.dir.name, "order")
        jobs = []
        for tier in (3, 1, 2):
            jobs.append(self.start("bash", "-c", f"echo {tier} >> {log}", LG_TIER=str(tier), LG_SETTLE="0", LG_POLL="0.05"))
            self.queued(jobs[-1])
        self.set_load(16.0, 16.0)
        for job in jobs:
            job.wait(timeout=15)
        self.assertEqual(open(log).read().split(), ["3", "1", "2"])  # by age, not tier
