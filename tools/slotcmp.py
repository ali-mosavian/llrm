#!/usr/bin/env python3
"""tools/slotcmp.py SRC.c LEVEL TARGET MODE [CC_ARGS...]: the allocator's slot-instruction count per function, ours against the pin's.

The same IR both sides: ours counts the instructions of the body the allocator gets (`LLRM_SLOT_COUNT=1`), the pin's
`llc -stop-before=greedy` prints the MIR of the IR our `--dump` hands the backend. Prints the functions that differ, the
histogram of ours minus the pin's, and the totals. Needs the pin's llc (`tools/pairs/pin.py`).
ABI-matched: at m32 llrm-cc's convention is regcall (registers) and the pin's llc has sysv; a size or count compared across them is void
(rectwo -O2 read +6 B above the pin, -8 B below it ABI-matched). Both sides here take the same IR with `x86_regcallcc` stripped, ours at `-mabi=sysv`.
Env: LLRM_CC (default target/release/llrm-cc), LLRM_LLC (default target/release/llrm-llc), WORK (a directory on disk, default ~/scratch).
Example: tools/slotcmp.py bench/jpeg_fixed/jpeg_fixed.c O2 x86_64-linux m64 -Ibench
"""
import collections
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools/pairs"))
import pin  # noqa: E402


def dso_local(text: str) -> str:
    """The pin's llc is PIC on x86-64 unless its symbols say otherwise; ours is not (`irguard`)."""
    out = []
    for line in text.splitlines():
        if line.startswith("define ") and not re.search(r"dso_local|internal|private", line.split("@")[0]):
            line = line.replace("define ", "define dso_local ", 1)
        elif line.startswith("declare ") and "@llvm." not in line and "dso_local" not in line:
            line = line.replace("declare ", "declare dso_local ", 1)
        elif re.match(r"@\S+ = (global|constant|common|weak|hidden)", line) and "dso_local" not in line:
            line = re.sub(r"^(@\S+ = )", r"\1dso_local ", line, count=1)
        out.append(line)
    return "\n".join(out) + "\n"


def strip_regcall(text: str) -> str:
    """The IR with the llrm-cc convention marks removed, as both llcs take it: sysv at m32."""
    return re.sub(r" llrm-[a-z-]*", "", text).replace(" nearcode", "").replace("cc 1003 ", "").replace("x86_regcallcc ", "")


def ours(src: str, level: str, target: str, mode: str, args: list[str], work: Path) -> dict[str, int]:
    cc = os.environ.get("LLRM_CC", str(ROOT / "target/release/llrm-cc"))
    env = dict(os.environ, LLRM_CC_TARGET=target, LLRM_SLOT_COUNT="1")
    env.setdefault("LLRM_LLC", str(ROOT / "target/release/llrm-llc"))
    run = subprocess.run([cc, f"-{level}", "-w", "-c", "-o", str(work / "o.o"), src, *args, "--dump", str(work / "d")], env=env, capture_output=True, text=True)
    if run.returncode:
        sys.exit(run.stderr)
    if mode == "m32":  # llrm-cc's own run is regcall: count again on the IR both sides share, at sysv
        ir = pin.backend_ir(work / "d") or sorted((work / "d").glob("[0-9][0-9]-*.ll"))[-1]
        (work / "sysv.ll").write_text(strip_regcall(ir.read_text()))
        llc = env["LLRM_LLC"]
        run = subprocess.run([llc, "-m32", f"-{level}", "-march=i486", "--isel=dag", "-mabi=sysv", "-S", "-o", os.devnull, str(work / "sysv.ll")], env=env, capture_output=True, text=True)
        if run.returncode:
            sys.exit(run.stderr)
    return {m[1]: int(m[2]) for m in re.finditer(r"^SLOTS (\S+) (\d+)$", run.stderr, re.M)}


def theirs(level: str, mode: str, work: Path) -> dict[str, int]:
    ir = pin.backend_ir(work / "d") or sorted((work / "d").glob("[0-9][0-9]-*.ll"))[-1]
    text = strip_regcall(dso_local(ir.read_text()))
    done = pin.llc(["-O2" if level == "Os" else f"-{level}", "-stop-before=greedy", "-o", str(work / "pin.mir")], text, pin.target(mode))
    if done.returncode:
        sys.exit(done.stderr)
    counts: dict[str, int] = {}
    name, body = None, False
    for line in (work / "pin.mir").read_text().splitlines():
        if m := re.match(r"name:\s+(\S+)", line):
            name, body = m[1], False
            counts[name] = 0
        elif line.startswith("body:"):
            body = True
        elif line.startswith("..."):
            body = False
        elif name and body and re.match(r"^    \S", line) and not line.startswith(("    bb.", "    successors:", "    liveins:")):
            counts[name] += 1
    return counts


def main(argv: list[str]) -> int:
    if len(argv) < 4:
        print(__doc__)
        return 2
    src, level, target, mode, *args = argv
    work = Path(tempfile.mkdtemp(prefix="slotcmp-", dir=os.environ.get("WORK", str(Path.home() / "scratch"))))  # a dump is not for tmpfs
    mine, pins = ours(src, level, target, mode, args, work), theirs(level, mode, work)
    both = sorted(set(mine) & set(pins))
    for name in both:
        if mine[name] != pins[name]:
            print(f"{name} ours {mine[name]} pin {pins[name]}")
    print("DELTAS", dict(sorted(collections.Counter(mine[n] - pins[n] for n in both).items())))
    print("TOTAL ours", sum(mine[n] for n in both), "pin", sum(pins[n] for n in both), "functions", len(both))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
