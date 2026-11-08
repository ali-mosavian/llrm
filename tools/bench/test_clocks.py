"""Clock readings: the CPU a timing is pinned to, and when a cell counts as a number."""

import os
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))
import clocks  # noqa: E402


def test_the_cpu_is_not_one_a_busy_process_holds_nor_its_hyperthread():
    """A fixed `taskset -c 6` timed other sessions' work on cpu 6: with it busy the pick must move off it and off its sibling."""
    cpus = sorted(os.sched_getaffinity(0))
    if 6 not in cpus or len(cpus) < 4:
        pytest.skip("needs cpu 6 and a few more")
    held = []
    try:
        for cpu in clocks.siblings(6):
            held.append(subprocess.Popen(["taskset", "-c", str(cpu), sys.executable, "-c", "while True: pass"]))
        pick = clocks.quiet_cpu()
        assert pick not in clocks.siblings(6), f"picked cpu {pick}, the core of busy cpu 6"
    finally:
        for process in held:
            process.kill()
            process.wait()


def test_the_quietest_core_wins_by_its_sibling_too():
    busy = {0: 0.0, 1: 0.9, 2: 0.2, 3: 0.2}
    clocks.siblings = lambda cpu: {0: {0, 1}, 1: {0, 1}, 2: {2, 3}, 3: {2, 3}}[cpu]
    assert clocks.quiet_cpu(busy) in (2, 3)


def test_a_cell_with_more_than_ten_percent_spread_is_unmeasured():
    """sfm16_float_i2f -O2 read 568 and 315 for one binary: a median of 4 placements printed as a figure."""
    noisy = clocks.summarize([315, 316, 568, 570])
    assert not noisy["measured"] and noisy["min"] == 315
    quiet = clocks.summarize([612, 615, 618, 620])
    assert quiet["measured"] and quiet["median"] == 616.5
