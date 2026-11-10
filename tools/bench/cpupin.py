"""Native clock readings on a shared machine: which CPU to pin to, and when a cell is a number.

Other sessions' builds run on the same cores. A fixed `taskset -c 6` read their load, not the code: one binary measured 315 and 631 cycles in
different runs, the compiler it was compared with 327 and 522 (sfm16_float_i2f -O2, 2026-11-08)."""

import time
from pathlib import Path

SPREAD_LIMIT = 10.0  # percent: a cell with more spread between placements than this is unmeasured


def _busy():
    """Per logical CPU, the share of the last 0.25 s it ran something."""
    def snap():
        taken = {}
        for line in Path("/proc/stat").read_text().splitlines():
            if line.startswith("cpu") and line[3].isdigit():
                fields = [int(x) for x in line.split()[1:9]]
                taken[int(line.split()[0][3:])] = (sum(fields) - fields[3] - fields[4], sum(fields))
        return taken

    before = snap()
    time.sleep(0.25)
    after = snap()
    return {cpu: (after[cpu][0] - before[cpu][0]) / max(1, after[cpu][1] - before[cpu][1]) for cpu in after}


def siblings(cpu):
    """The logical CPUs sharing `cpu`'s core, itself included."""
    text = Path(f"/sys/devices/system/cpu/cpu{cpu}/topology/thread_siblings_list").read_text().strip()
    cpus = set()
    for part in text.split(","):
        low, _, high = part.partition("-")
        cpus.update(range(int(low), int(high or low) + 1))
    return cpus


def quiet_cpu(busy=None):
    """The CPU whose core, it and its sibling, was least busy."""
    busy = busy or _busy()
    return min(busy, key=lambda cpu: (sum(busy.get(other, 0.0) for other in siblings(cpu)), cpu))


def summarize(cycles):
    """The median, minimum and maximum of the per-placement cycles, the spread in percent of the median, and whether the cell is a number.

    A cell is `measured` only if its spread is within SPREAD_LIMIT; a noisy one reports `unmeasured`, never a figure to compare."""
    if not cycles:
        raise ValueError("no placements measured")
    ordered = sorted(cycles)
    middle = len(ordered) // 2
    median = ordered[middle] if len(ordered) % 2 else (ordered[middle - 1] + ordered[middle]) / 2
    spread = 100.0 * (ordered[-1] - ordered[0]) / median
    return {"median": median, "min": ordered[0], "max": ordered[-1], "spread": spread, "measured": spread <= SPREAD_LIMIT}


if __name__ == "__main__":
    import sys

    if sys.argv[1:2] == ["cpu"]:
        print(quiet_cpu())
    else:
        out = summarize([float(a) for a in sys.argv[1:]])
        print(f"{out['median']:.0f} {out['min']:.0f} {out['max']:.0f} {out['spread']:.1f} {'measured' if out['measured'] else 'unmeasured'}")
