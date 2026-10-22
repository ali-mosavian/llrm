#!/usr/bin/env python3
"""pin_noop.py: does the pin's pass change our IR? One run of each N/A pass of the slot table over the IRs of `LLRM_ABLATE_IRS`.

    LLRM_ABLATE_IRS=~/scratch/fa/pinir tools/pairs/pin_noop.py

Each pass runs alone through tools/pairs/pin.py (the m32 target: i386, -march=i486, no SSE) over every */01-globalopt.ll as the pin's
`default<O2>` leaves it (the IR a late pass sees: an unoptimized one gives a vectorizer no loop), and its `-S` output is compared with
`-passes=verify`'s. The O2 outputs also count vector types: none means the vectorizers had nothing to make on this target. Prints, per pass, how many IRs it changed. A 0 over these IRs is the evidence that the pass
has no work on them (the slot table's N/A rows); it says nothing about IR the corpus does not have (coroutines, OpenMP, profile data).
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import pin  # noqa: E402

# pass name in the slot table -> the pipeline text that runs it alone
PASSES = {
    "ee-instrument": "function(ee-instrument)",
    "speculative-execution": "function(speculative-execution<only-if-divergent-target>)",
    "vector-combine": "function(vector-combine)",
    "move-auto-init": "function(move-auto-init)",
    "coro-elide": "function(coro-elide)",
    "inject-tli-mappings": "function(inject-tli-mappings)",
    "loop-vectorize": "function(loop-vectorize)",
    "slp-vectorizer": "function(slp-vectorizer)",
    "transform-warning": "function(transform-warning)",
    "annotation-remarks": "function(annotation-remarks)",
    "chr": "function(chr)",
    "memprof-remove-attributes": "memprof-remove-attributes",
    "annotation2metadata": "annotation2metadata",
    "forceattrs": "forceattrs",
    "coro-early": "coro-early",
    "coro-split": "cgscc(coro-split)",
    "coro-annotation-elide": "cgscc(coro-annotation-elide)",
    "coro-cleanup": "coro-cleanup",
    "openmp-opt": "openmp-opt",
    "openmp-opt-cgscc": "cgscc(openmp-opt-cgscc)",
    "alloc-token": "alloc-token",
    "cg-profile": "cg-profile",
    "rel-lookup-table-converter": "rel-lookup-table-converter",
    "lower-comment-string": "lower-comment-string",
}


def controls(irs, texts, of) -> None:
    """The instrument's own checks: a pass that must change something does, and the vectorizers do once the target has SSE."""
    early = sum(pin.opt("verify", ir.read_text(), of).stdout != pin.opt("function(early-cse)", ir.read_text(), of).stdout for ir in irs)
    sse = pin.Target(of.triple, "pentium4", of.datalayout, "+sse,+sse2")
    vectorized = sum(
        pin.opt("verify", texts[ir], sse).stdout != pin.opt("function(loop-vectorize,slp-vectorizer)", texts[ir], sse).stdout for ir in irs
    )
    print(f"# control: early-cse changes {early} of {len(irs)} unoptimized IRs; with SSE on (pentium4) the vectorizers change {vectorized} of {len(irs)} default<O2> outputs")


def main() -> int:
    root = Path(os.environ.get("LLRM_ABLATE_IRS", Path.home() / "scratch/fa/pinir")).expanduser()
    irs = sorted(root.glob("*/01-globalopt.ll"))
    of = pin.target("m32")
    base = {}
    texts = {}
    vectors = 0
    for ir in irs:
        optimized = pin.opt("default<O2>", ir.read_text(), of)
        if optimized.returncode != 0:
            print("default<O2> failed on", ir, optimized.stderr[:200])
            return 1
        vectors += sum(line.count(" x ") for line in optimized.stdout.splitlines() if "<" in line and "> " in line and "x " in line)
        texts[ir] = optimized.stdout
        done = pin.opt("verify", texts[ir], of)
        if done.returncode != 0:
            print("verify failed on", ir, done.stderr[:200])
            return 1
        base[ir] = done.stdout
    controls(irs, texts, of)
    print(f"# pin opt, one pass alone over {len(irs)} IRs after default<O2> ({root}); target m32; `changed` = output differs from -passes=verify's")
    print(f"# vector types in the {len(irs)} default<O2> outputs: {vectors}")
    print("pass | pipeline | IRs changed | note")
    for name, pipeline in PASSES.items():
        changed = failed = 0
        why = ""
        for ir in irs:
            done = pin.opt(pipeline, texts[ir], of)
            if done.returncode != 0:
                failed += 1
                why = done.stderr.strip().splitlines()[-1][:80] if done.stderr.strip() else "failed"
                continue
            if done.stdout != base[ir]:
                changed += 1
        print(f"{name} | {pipeline} | {changed} | {('opt failed on ' + str(failed) + ': ' + why) if failed else ''}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
