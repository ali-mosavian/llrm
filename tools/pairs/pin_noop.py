#!/usr/bin/env python3
"""pin_noop.py: does the pin's pass do anything AT ITS SLOT? The N/A rows of the slot table, tested.

    LLRM_NOOP_SETS="m32:DIR m16:DIR" tools/pairs/pin_noop.py

For each IR (`DIR/*/01-globalopt.ll`: the unoptimized IR of the bench C programs, or of the QCport modules from the pin's own clang) the pin's
`default<O2>` pipeline as printed (tests/fixtures/pin-pipeline/O2.txt) runs whole and with every instance of the pass removed
(`-passes=<printed list minus X>`), through tools/pairs/pin.py with the set's target; the final IR text is compared. Identical for every
IR is the evidence that the pass has no work at its slot (N/A); one that differs is MISSING. -Os is the same list on optsize IR.
Controls, printed first: removing `sroa` changes the IR of every program, and with SSE on removing the vectorizers does too.
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import pin  # noqa: E402
import pin_ablate  # noqa: E402

# the passes of the slot table's N/A rows (bare names as the printed pipeline spells them)
PASSES = (
    "ee-instrument speculative-execution vector-combine move-auto-init coro-elide inject-tli-mappings loop-vectorize slp-vectorizer "
    "transform-warning annotation-remarks chr memprof-remove-attributes annotation2metadata forceattrs coro-early coro-split "
    "coro-annotation-elide coro-cleanup openmp-opt openmp-opt-cgscc recompute-globalsaa alloc-token cg-profile "
    "rel-lookup-table-converter lower-comment-string"
).split()


def final(pipeline: str, text: str, of) -> str | None:
    done = pin.opt(pipeline, pin_ablate.pin_text_of(text), of)
    return done.stdout if done.returncode == 0 else None


def main() -> int:
    sets = []
    for spec in os.environ.get("LLRM_NOOP_SETS", f"m32:{Path.home()}/scratch/fa/pinir").split():
        mode, directory = spec.split(":", 1)
        sets.append((mode, sorted(Path(directory).expanduser().glob("*/01-globalopt.ll"))))
    for level, optsize in (("O2", False), ("Os", True), ("O3", False)):
        fixture = pin_ablate.FIXTURE.with_name("O3.txt" if level == "O3" else "O2.txt")
        line = [l for l in fixture.read_text().splitlines() if not l.startswith(";")][-1]
        root = pin_ablate.tree(line)
        full = pin_ablate.show(root)
        results: dict[str, list[int]] = {name: [0, 0] for name in ["sroa", "vectorizers-with-SSE", *PASSES]}
        total = 0
        for mode, irs in sets:
            of = pin.target(mode, optsize=optsize)
            sse = pin.Target(of.triple, "pentium4", of.datalayout, "+sse,+sse2", optsize)
            for ir in irs:
                text = ir.read_text()
                base = final(full, text, of)
                if base is None:
                    continue
                total += 1
                for name in [*PASSES, "sroa"]:
                    out = final(pin_ablate.show(pin_ablate.without(root, name)), text, of)
                    results[name][0] += out != base
                    results[name][1] += out is None
                a = final(full, text, sse)
                b = final(pin_ablate.show(pin_ablate.without(pin_ablate.without(root, "loop-vectorize"), "slp-vectorizer")), text, sse)
                results["vectorizers-with-SSE"][0] += a != b
        print(f"# {level}{' (optsize IR)' if optsize else ''}: {total} IRs the pin parses ({', '.join(f'{mode} {len(irs)}' for mode, irs in sets)} listed); pin d1106deb71cc")
        print("pass | IRs whose final IR differs without it | opt failed")
        for name, (changed, failed) in results.items():
            print(f"{name} | {changed} | {failed}")
        sys.stdout.flush()
    return 0


if __name__ == "__main__":
    sys.exit(main())
