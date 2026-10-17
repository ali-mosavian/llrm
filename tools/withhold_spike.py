#!/usr/bin/env python3
"""S1 step 0: which rewrites does a whole-function fact enable? (measurement only)

    QCPORT=<qb-quake src> tools/withhold_spike.py [--llrm llrm-c] [--level O2] [--passes fold,decide,...] [source.c]

Compiles with `LLRM_WITHHOLD` naming a pass (and facts): the pass is handed the conservative result in place of the real
registers, through-memory, bounds and followers (`Analysis::withheld`). Per pass it prints the edits it logged with and without
(`[counts] edits`), the results withheld, the compile's instructions and whether the object differs. Controls: a name no pass has
must give the same object; `all` must leave the edits of the passes that read no such fact alone. `LLRM_WITHHOLD_FIRST` cuts to
each pass's first run, where the input is the unwithheld run's.
"""
from __future__ import annotations

import argparse
import collections
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
KEYWORDS = [f"-D{k}=" for k in "far huge near pascal cdecl __far _far __near _near".split()]


def compiled(llrm: str, level: str, source: str, extra: list[str], env: dict[str, str]) -> tuple[dict, bytes, int]:
    """(edits by pass, the object, instructions) of one compile."""
    with tempfile.TemporaryDirectory() as tmp:
        obj = Path(tmp) / "o.obj"
        done = subprocess.run(
            ["perf", "stat", "-x,", "-e", "instructions:u", llrm, "-m32", "-march=i486", f"-{level}", *extra, "-o", str(obj), source],
            capture_output=True, text=True, env={**os.environ, "LLRM_DEBUG": "counts", **env},
        )
        if done.returncode:
            sys.exit(f"failed with {env}: {done.stderr[-600:]}")
        edits, withheld = collections.Counter(), collections.Counter()
        for line in done.stderr.splitlines():
            if one := re.fullmatch(r"\[counts\] edits (\S+) (\S+) (\d+)", line):
                edits[one[1]] += int(one[3])
            elif one := re.fullmatch(r"\[counts\] withheld (\S+) (\S+) (\d+)", line):
                withheld[f"{one[1]}:{one[2]}"] += int(one[3])
        instr = int(re.search(r"^(\d+),,instructions", done.stderr, re.M)[1])
        return {"edits": edits, "withheld": withheld}, obj.read_bytes(), instr


FACTS = ("registers", "through-memory", "bounded", "followers", "annotated", "exposed-frames", "float-facts")


def pass_work(llrm: str, level: str, source: str, extra: list[str]) -> dict[str, float]:
    """Each pass's total Minstr (the analyses it asked for inside it) at baseline."""
    sys.path.insert(0, str(ROOT / "tools"))
    import pass_budgets

    done = subprocess.run(
        [llrm, "-m32", "-march=i486", f"-{level}", *extra, "-o", "/tmp/withhold-work.obj", source],
        capture_output=True, text=True, env={**os.environ, "LLRM_DEBUG": "counts,time", "LLRM_TIME_TOP": "2000"},
    )
    return {one["pass"]: one["total"] for one in pass_budgets.parsed(done.stderr)}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("source", nargs="?")
    parser.add_argument("--llrm", default=str(ROOT / "target/release/llrm-c"))
    parser.add_argument("--level", default="O2")
    parser.add_argument("--facts", default=",".join(FACTS))
    parser.add_argument("--passes", default="fold,decide,promote,sroa,hoist,gvn,dse,algebraic,indvars,jumpthread,lsr")
    args = parser.parse_args()
    if args.source:
        source, extra = args.source, []
    else:
        qc = Path(os.environ["QCPORT"]).expanduser()
        source = str(qc / "render/d_faces.c")
        extra = KEYWORDS + [flag for d in ("", "render", "game", "host", "qgl") for flag in ("-I", str(qc / d))]
    base, base_obj, base_instr = compiled(args.llrm, args.level, source, extra, {})
    work = pass_work(args.llrm, args.level, source, extra)
    print(f"-{args.level} baseline: {sum(base['edits'].values())} edits, {base_instr / 1e6:.0f} M instructions, object {len(base_obj)} bytes")
    nothing, nothing_obj, _ = compiled(args.llrm, args.level, source, extra, {"LLRM_WITHHOLD": "no-such-pass"})
    print(f"control (a name no pass has): object {'identical' if nothing_obj == base_obj else 'DIFFERENT'}, edits {'equal' if nothing['edits'] == base['edits'] else 'DIFFER'}")
    print(f"{'cut':9} {'pass':11} {'fact':14} {'edits':>6} {'enabled':>8} {'%':>6} {'asks':>5} {'bytes':>6} {'object':>7} {'cost M':>7} {'of pass':>8}")
    for mode, env in (("all runs", {}), ("first run", {"LLRM_WITHHOLD_FIRST": "1"})):
        for name in args.passes.split(","):
            for fact in ("", *args.facts.split(",")):
                spec = f"{name}:{fact}" if fact else name
                got, obj, instr = compiled(args.llrm, args.level, source, extra, {"LLRM_WITHHOLD": spec, **env})
                asks = sum(got["withheld"].values())
                if not asks:
                    continue
                was, now = base["edits"][name], got["edits"][name]
                cost = (base_instr - instr) / 1e6
                print(
                    f"{mode:9} {name:11} {fact or 'all four':14} {was:6} {was - now:+8} {100 * (was - now) / max(was, 1):5.1f}% {asks:5} "
                    f"{len(obj) - len(base_obj):+6} {'same' if obj == base_obj else 'differs':>7} {cost:7.1f} {100 * cost / max(work.get(name, 1e-9), 1e-9):7.1f}%"
                )


if __name__ == "__main__":
    main()
