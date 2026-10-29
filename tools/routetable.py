"""Per allocation and per function: what the trials and the second route bought (docs/optimizations/single-allocation-decisions.md).

    tools/routetable.py [-O1,-O2,-Os]      writes /tmp/routetable.json; needs QCPORT and QCPORT_INC, as tools/identical.py
"""
import json
import os
import re
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
import llrmbin  # noqa: E402

BASE = re.compile(r"\[regalloc\] (\S+): (\d+) insns, (\d+) spilled, cost ([\d.e+-]+), (\d+) forced")
TRIAL = re.compile(r"\[regalloc\]   trial (\w+) \(splitting (\w+)\): (\d+) spilled, cost ([\d.e+-]+), (\d+) forced")
KEPT = re.compile(r"\[regalloc\]   kept the trial")
FEATURES = re.compile(
    r"\[routes\] (\S+): all ([\d.e+-]+) traffic ([\d.e+-]+) spill ([\d.e+-]+) loop-spill ([\d.e+-]+) "
    r"frame-cells ([\d.e+-]+) spill-insns (\d+) insns (\d+)"
)
CANDIDATES = re.compile(r"\[candidates\] (\S+): spiller (\S+) / (\S+), allocator alone (\S+) / (\S+)")


def priced(text):
    found = re.match(r"Some\(([\d.e+-]+)\)", text)
    return float(found.group(1)) if found else None


def parse(name, lvl, err):
    """The rows of one compile's stderr: one per allocation run, one per function whose two routes were priced."""
    rows = []
    runs = []
    cur = None
    feat = None
    for l in err.splitlines():
        m = BASE.match(l)
        if m:
            cur = {"fn": m.group(1), "insns": int(m.group(2)), "spilled": int(m.group(3)), "base": float(m.group(4)), "trials": [], "kept": None}
            runs.append(cur)
            continue
        m = TRIAL.match(l)
        if m and cur:
            cur["trials"].append((m.group(1), m.group(2) == "true", int(m.group(3)), float(m.group(4))))
            continue
        if KEPT.match(l) and cur and cur["trials"]:
            cur["kept"] = len(cur["trials"]) - 1
            continue
        m = FEATURES.match(l)
        if m:
            feat = {"fn": m.group(1), "all": float(m.group(2)), "traffic": float(m.group(3)), "spill": float(m.group(4)), "loopspill": float(m.group(5)), "frame": float(m.group(6)), "spillinsns": int(m.group(7)), "insns": int(m.group(8)), "alone_run": len(runs) - 1}
            continue
        m = CANDIDATES.match(l)
        if m and feat:
            rows.append({"file": name, "level": lvl, "kind": "route", "fn": m.group(1), "spiller": priced(m.group(2)), "alone": priced(m.group(4)), **feat})
    rows.extend({"file": name, "level": lvl, "kind": "run", **r} for r in runs)
    return rows


def compiled(job):
    name, source, flags, level = job
    environment = dict(os.environ, LLRM_DEBUG="regalloc,candidates,routes", LLRM_CANDIDATES="measure")
    command = [str(llrmbin.compiler(ROOT / "target/release")), "-c", *llrmbin.c_args(16), *flags, level, "-S", "-o", "/dev/null", str(source)]
    done = subprocess.run(command, capture_output=True, text=True, env={**environment, **llrmbin.cc_environment()}, timeout=900)
    return name, level, done.stderr


def main():
    import identical

    levels = sys.argv[1].split(",") if len(sys.argv) > 1 else ["-O1", "-O2", "-Os"]
    work = Path(tempfile.mkdtemp(prefix="routetable-"))
    corpus = identical.corpus(work, Path(os.environ["QCPORT"]), Path(os.environ["QCPORT_INC"]))
    jobs = [(name, source, flags, level) for name, (source, flags) in corpus.items() for level in levels]
    rows = []
    with ThreadPoolExecutor(8) as pool:
        for name, level, err in pool.map(compiled, jobs):
            rows.extend(parse(name, level, err))
    json.dump(rows, open("/tmp/routetable.json", "w"))
    print(len(rows), "rows;", len(jobs), "compiles")


if __name__ == "__main__":
    main()
