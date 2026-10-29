"""Register copies a function has after phi elimination, two-address and the coalescer: the pin's `COPY`s and ours,
and why each of ours was left (`LLRM_COALESCE_WHY=1`).

usage: copywhy.py LLRM_CC LLC OUT_DIR [--triple T [--cpu C]] FUNCTION... -- llrm-cc args (-c, target, source, flags, includes)

The pin runs as code16 with a frame pointer unless `--triple` names its target (m32: `--triple i386-unknown-linux-gnu --cpu i486`).

Ours counts `dst := mov src` with a register destination in `ISEL_DUMP=1`; a store of a register is no copy. The pin
runs `llc -O2` on the dump's `*-cgpaddr.ll` (llc has no -Os). The reasons come from a probe on the coalescer's
final state, so a probe that joins changes the state the next sees: a count, not a replay.
"""
import collections
import glob
import os
import re
import subprocess
import sys

STAGES = ("PhiElimination", "TwoAddress", "Coalescer")


CODE16 = ["-mtriple=i386-pc-linux-code16", "-frame-pointer=all"]


def pin_counts(llc: str, ll: str, target: list[str] = CODE16) -> dict:
    run = subprocess.run(
        [llc, *target, "-O2",
         "-print-after=phi-node-elimination,twoaddressinstruction,register-coalescer", ll, "-o", "/dev/null"],
        capture_output=True, text=True)
    secs = re.split(r"(?m)^# \*\*\* IR Dump After (.*?) \*\*\*:\n", run.stderr)
    out = {}
    for i in range(1, len(secs), 2):
        fn = re.match(r"# Machine code for function (\S+?):", secs[i + 1]).group(1)
        stage = {"Eliminate": STAGES[0], "Two-Address": STAGES[1], "Register": STAGES[2]}[secs[i].split()[0]]
        out[(stage, fn)] = len(re.findall(r"(?m)^\s*(?:[^\n=]*= )?COPY\b", secs[i + 1]))
    return out


def our_counts(text: str) -> dict:
    stage = fn = None
    out = collections.Counter()
    for line in text.splitlines():
        if m := re.match(r"=== (\w+)", line):
            stage = m.group(1)
        elif m := re.match(r"  (\S+?)(?:@\d+)?: \d+ instructions", line):
            fn = m.group(1)
        elif fn and re.match(r"\s+0x\w+\s+[A-Za-z]\w*(@\w+)? := mov [A-Za-z][\w@]*\s*(;|$)", line):
            out[(stage, fn)] += 1
    return out


def split_target(names: list[str]) -> tuple[list[str], list[str]]:
    """The function names, and the pin's target flags from `--triple T [--cpu C]` among them."""
    names, target = list(names), CODE16
    if "--triple" in names:
        at = names.index("--triple")
        target = [f"-mtriple={names[at + 1]}"]
        del names[at : at + 2]
        if "--cpu" in names:
            at = names.index("--cpu")
            target.append(f"-mcpu={names[at + 1]}")
            del names[at : at + 2]
    return names, target


def main() -> None:
    llrm, llc, out, *rest = sys.argv[1:]
    cut = rest.index("--")
    funcs, args = rest[:cut], rest[cut + 1:]
    funcs, target = split_target(funcs)
    os.makedirs(out, exist_ok=True)
    env = dict(os.environ, ISEL_DUMP="1", LLRM_COALESCE_WHY="1")
    run = subprocess.run([llrm, *args, "--dump", out, "-o", os.path.join(out, "x.obj")], env=env, capture_output=True, text=True)
    assert run.returncode == 0, run.stderr[-2000:]
    ours = our_counts(run.stdout + run.stderr)
    why = collections.defaultdict(collections.Counter)
    for line in run.stderr.splitlines():
        if line.startswith("WHY\t"):
            _, fn, reason = line.split("\t")
            why[fn][reason] += 1
    pin = pin_counts(llc, glob.glob(os.path.join(out, "*-cgpaddr.ll"))[0], target)
    print("pin = llc -O2 on the cgpaddr dump; ours = llrm-cc as run (see header of tools/copywhy.py)")
    print("function | phi pin/ours | twoaddr pin/ours | coalescer pin/ours | why ours were left")
    for f in funcs:
        cells = [f"{pin.get((s, f))}/{ours.get((s, f))}" for s in STAGES]
        left = ", ".join(f"{n} {r}" for r, n in why[f].most_common()) or "none"
        print(f, *cells, left, sep=" | ")


if __name__ == "__main__":
    main()
