"""A pytest plugin: a test that uses more than GATE_TEST_CPU_SECONDS (600) of CPU, its subprocesses' included, fails with its name.

pytest-timeout limits wall time, which a loaded host stretches; this counts CPU. A test that waits on nothing and uses no CPU is the stall
detector's (gate.py run_step)."""
import os
import signal
import sys
import threading

import pytest

import proctree

LIMIT = float(os.environ.get("GATE_TEST_CPU_SECONDS", "600"))


class CpuLimitExceeded(Exception):
    pass


@pytest.hookimpl(hookwrapper=True)
def pytest_runtest_call(item):
    me = os.getpid()
    start = proctree.tree_cpu(me)
    done = threading.Event()
    fired = []

    def raise_in_test(signum, frame):
        raise CpuLimitExceeded(fired[0])

    def watch():
        while not done.wait(0.25):
            used = proctree.tree_cpu(me) - start
            if used > LIMIT:
                fired.append(f"{item.nodeid} used {used:.0f} s of CPU, over the limit of {LIMIT:g} s (GATE_TEST_CPU_SECONDS)")
                print(f"\nCPU LIMIT: {fired[0]}", file=sys.stderr, flush=True)
                for child in proctree.descendants(me):
                    try:
                        os.kill(child, signal.SIGKILL)
                    except OSError:
                        pass
                os.kill(me, signal.SIGUSR1)
                return

    previous = signal.signal(signal.SIGUSR1, raise_in_test)
    watcher = threading.Thread(target=watch, daemon=True)
    watcher.start()
    try:
        yield
    finally:
        done.set()
        watcher.join()
        signal.signal(signal.SIGUSR1, previous)
