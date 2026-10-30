#!/usr/bin/env python3
"""pass_ir_diff.py PASS... : the pin's pass and ours on one input IR, functions whose output differs.

    LLRM_ABLATE_IRS=DIR LLRM_LLRM_PASS=target/release/llrm-pass tools/pairs/pass_ir_diff.py instcombine simplifycfg ...

The input of a pass is the pin's own output after the passes that precede it at its slot (PREFIX), so both sides start from the same IR. Each side's
output is reduced per function to a multiset of instruction shapes (opcode, compare predicate, result type; names, flags and operands dropped); a
function whose two multisets differ is a diff. Prints, per pass, the count and the five smallest diffs with the shapes each side has more of.
"""
from __future__ import annotations

import collections
import os
import re
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import pin  # noqa: E402
import pin_ablate  # noqa: E402

# the function simplification pipeline's SimplifyCFG (PassBuilderPipelines.cpp:675), as the printed pipeline spells it
CFG = (
    "simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;"
    "no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables>"
)

# pass: (the pin's pipeline before it, the pin's spelling, ours: llrm-pass arguments)
PREFIX = {
    "sroa": ("function(simplifycfg)", "function(sroa<modify-cfg>)", ["--order", "sroa@646"]),
    "early-cse": ("function(simplifycfg,sroa<modify-cfg>)", "function(early-cse<>)", ["--order", "earlycse@649"]),
    "instcombine": (
        "function(simplifycfg,sroa<modify-cfg>,early-cse<>,jump-threading,correlated-propagation," + CFG + ")",
        "function(instcombine<no-verify-fixpoint>)",
        ["--order", "combine@677"],
    ),
    "simplifycfg": (
        "function(sroa<modify-cfg>,early-cse<>,jump-threading,correlated-propagation)",
        "function(" + CFG + ")",
        ["--order", "decide@675"],
    ),
    "gvn": (
        "function(simplifycfg,sroa<modify-cfg>,early-cse<>,instcombine<no-verify-fixpoint>," + CFG + ",loop-simplify,lcssa,loop-mssa(licm<allowspeculation>))",
        "function(gvn<>)",
        ["--order", "gvn@787"],
    ),
    "licm": (
        "function(simplifycfg,sroa<modify-cfg>,early-cse<>,instcombine<no-verify-fixpoint>," + CFG + ",loop-simplify,lcssa)",
        "function(loop-mssa(licm<allowspeculation>))",
        ["--order", "licm@724"],
    ),
}

SHAPE = re.compile(r"^\s*(?:%[^=]+=\s*)?(?P<op>[a-z_.]+)(?: (?:nuw |nsw |exact |disjoint |samesign |inbounds |nneg |fast |nnan |ninf |tail |notail |musttail )*)?(?P<rest>.*)$")


def functions(text: str) -> dict[str, collections.Counter]:
    """`text` as {function: Counter of instruction shapes}."""
    out: dict[str, collections.Counter] = {}
    current = None
    for line in text.splitlines():
        if line.startswith("define"):
            name = re.search(r"@(\"[^\"]+\"|[^( ]+)\(", line)
            current = name.group(1) if name else line
            out[current] = collections.Counter()
        elif line.startswith("}"):
            current = None
        elif current and line.startswith("  ") and not line.strip().startswith(";"):
            m = SHAPE.match(line)
            if not m:
                continue
            op, rest = m.group("op"), m.group("rest").strip()
            if op in ("icmp", "fcmp"):
                op += " " + rest.split()[0]
            elif op == "call":
                callee = re.search(r"@(\"[^\"]+\"|[\w.$]+)\(", rest)
                op += " @" + (callee.group(1) if callee else "?")
            elif op in ("load", "store"):
                op += " " + rest.split(",")[0].split()[0]
            elif op in ("br",):
                op += " cond" if rest.startswith("i1") else ""
            out[current][op] += 1
    return out


def ours_text(text: str) -> str:
    return "\n".join(l for l in text.splitlines() if not l.startswith(("source_filename", "target triple", "; ")))


def run(ir: Path, name: str, of, llrm_pass: str, level: str):
    prefix, spelling, args = PREFIX[name]
    start = pin.opt(prefix, pin_ablate.pin_text(ir), of)
    if start.returncode:
        return ir, None, f"prefix: {start.stderr.strip().splitlines()[:1]}"
    pin_out = pin.opt(spelling, start.stdout, of)
    if pin_out.returncode:
        return ir, None, f"pin: {pin_out.stderr.strip().splitlines()[:1]}"
    tmp = Path(os.environ.get("TMPDIR", "/tmp")) / f"pid-{os.getpid()}-{ir.parent.name}-{name}.ll"
    tmp.write_text(ours_text(start.stdout))
    cmd = [llrm_pass, level, "--mode", "16" if of.triple.endswith("code16") else "32", *args, str(tmp)]
    ours = subprocess.run(cmd, capture_output=True, text=True)
    tmp.unlink(missing_ok=True)
    if ours.returncode:
        return ir, None, f"ours: {ours.stderr.strip().splitlines()[-1:]}"
    return ir, (functions(pin_out.stdout), functions(ours.stdout)), ""


def main(passes: list[str]) -> int:
    root = Path(os.environ.get("LLRM_ABLATE_IRS", Path.home() / "scratch/fa/qcir")).expanduser()
    irs = sorted(root.glob("*/01-globalopt.ll"))
    of = pin.target(os.environ.get("LLRM_ABLATE_TARGET", "m16"), optsize=bool(os.environ.get("LLRM_ABLATE_OPTSIZE", "1")))
    llrm_pass = os.environ.get("LLRM_LLRM_PASS", "target/release/llrm-pass")
    level = "-Os" if of.optsize else "-O2"
    for name in passes:
        diffs, skipped, total = [], [], 0
        with ThreadPoolExecutor(int(os.environ.get("LLRM_JOBS", "6"))) as pool:
            for ir, pair, why in pool.map(lambda one: run(one, name, of, llrm_pass, level), irs):
                if pair is None:
                    skipped.append(f"{ir.parent.name}: {why}")
                    continue
                theirs, mine = pair
                for fn in sorted(set(theirs) | set(mine)):
                    total += 1
                    a, b = theirs.get(fn, collections.Counter()), mine.get(fn, collections.Counter())
                    if a != b:
                        diffs.append((sum(a.values()) + sum(b.values()), ir.parent.name, fn, a - b, b - a))
        diffs.sort(key=lambda d: d[0])
        print(f"## {name}: {len(diffs)} of {total} functions differ over {len(irs) - len(skipped)} IRs ({root}); skipped {len(skipped)}")
        for size, ir, fn, pin_more, ours_more in diffs[:5]:
            print(f"- {ir} {fn} (size {size}): pin has more {dict(pin_more)}; ours has more {dict(ours_more)}")
        sigs = collections.Counter(
            (tuple(sorted(a)), tuple(sorted(b))) for _, _, _, a, b in diffs if len(a) + len(b) <= 3
        )
        print("most frequent small shape differences (pin has more | ours has more): count")
        for (a, b), n in sigs.most_common(8):
            print(f"  {list(a)} | {list(b)}: {n}")
        for one in skipped[:3]:
            print(f"  skipped {one}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
