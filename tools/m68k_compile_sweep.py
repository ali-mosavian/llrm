#!/usr/bin/env python3
"""Which corpus programs llrm-cc fails to compile for m68k, at every level and CPU: names, with the cause.

m68k_compile_sweep.py [--cpus 68000,68020,68040,68060] [--levels O0,O1,O2,O3,Os,Oz] [--jobs N] --out DIR [NAME...]

The corpus is tools/m68k_corpus_matrix.py's (tests/run/c plus bench). Each applicable program is built the way the matrix builds `ours`
(llrm-cc -S, gas with --fatal-warnings). Verdicts: ok; cannot-select (a "Cannot select" from the backend); panic; refused (llrm-cc
says so); timeout; assemble-fail (gas rejects our text); compile-fail (anything else). A program `// targets:` leaves out of the CPU is n/a
by target: not a cell, counted apart. The result line says "N cells NOT TESTED" with names, as tools/pairs/objsweep.py does: a cell that
never ran is unknown, not clean. OUT gets `m68k-compile-sweep.tsv`, `not-tested.tsv` and the summary.
Environment: LLRM_CC, GCC_M68K, LLRM_PIN_CLANG, LLVM_M68K (as the matrix; the gcc and clang ones are unused here).
"""
import argparse
import re
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import m68k_corpus_matrix as mx  # noqa: E402

LEVELS = ("O0", "O1", "O2", "O3", "Os", "Oz")


def classify(stage: str, timed_out: bool, stderr: str) -> str:
    """The verdict of a failed build. `stage` is `llrm-cc` or `as`."""
    if timed_out:
        return "timeout"
    if stage == "as":
        return "assemble-fail"
    if re.search(r"cannot select", stderr, re.I):
        return "cannot-select"
    if re.search(r"panicked|stack overflow|SIGSEGV|assertion", stderr, re.I):
        return "panic"
    if re.search(r"refus", stderr, re.I):
        return "refused"
    return "compile-fail"


def build(tools: mx.Tools, cpu: str, level: str, path: Path, work: Path) -> tuple[str, str]:
    """(verdict, cause) of llrm-cc on one program."""
    flags = mx.o0_sweep.flags_of(path.read_text(), f"m68k-{cpu}")
    out = str(work / f"{path.parent.name}_{path.stem}.{cpu}.{level}")
    env = {**mx.os.environ, "LLRM_CC_TARGET": f"m68k-{cpu}"}
    for at, cmd in enumerate(tools.commands("ours", cpu, level, str(path), flags, out)):
        ok, timed, err = tools.run(cmd, env if at == 0 else None)
        if not ok:
            lines = [l for l in err.splitlines() if l.strip()]
            hits = [l for l in lines if re.search(r"error|refus|panick|cannot|unsupported", l, re.I)]
            return classify("llrm-cc" if at == 0 else "as", timed, err), ((hits or lines or ["no output"])[0])[:200]
        if at == 0:
            Path(out + ".s").write_text(".text\n" + mx.relaxing_branches(Path(out + ".s").read_text()))
    return "ok", ""


def summary(verdicts: dict[str, int], not_tested: list[tuple[str, str]], n_cells: int, stamp: str) -> str:
    """The result line."""
    parts = ", ".join(f"{k} {v}" for k, v in sorted(verdicts.items()))
    names = ", ".join(sorted({p for _, p in not_tested}))
    return f"{n_cells} cells tested ({parts}); {len(not_tested)} cells NOT TESTED (n/a by // targets:): {names}\n{stamp}"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cpus", default=",".join(mx.CPUS))
    parser.add_argument("--levels", default=",".join(LEVELS))
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--out", required=True)
    parser.add_argument("names", nargs="*")
    args = parser.parse_args()
    cpus, levels = args.cpus.split(","), args.levels.split(",")
    unknown = [x for x in cpus if x not in mx.CPUS] + [x for x in levels if x not in LEVELS]
    if unknown:
        raise SystemExit(f"unknown cpu or level: {unknown}")
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    tools = mx.Tools()
    corpus, _ = mx.programs()
    chosen = [(i, p) for i, p in corpus if not args.names or i in args.names or p.stem in args.names]
    jobs, not_tested = [], []
    for cpu in cpus:
        for ident, path in chosen:
            if mx.ct.applies(f"m68k-{cpu}", path.read_text()):
                jobs += [(ident, path, cpu, level) for level in levels]
            else:
                not_tested += [(f"m68k-{cpu} {level}", ident) for level in levels]
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        with ThreadPoolExecutor(args.jobs) as pool:
            results = list(pool.map(lambda j: build(tools, j[2], j[3], j[1], work), jobs))
    verdicts: dict[str, int] = {}
    rows = []
    for (ident, _, cpu, level), (verdict, cause) in zip(jobs, results):
        verdicts[verdict] = verdicts.get(verdict, 0) + 1
        rows.append(f"m68k-{cpu}\t{ident}\t{level}\t{verdict}\t{cause}")
    (out / "m68k-compile-sweep.tsv").write_text("target\tprog\tlevel\tverdict\tcause\n" + "\n".join(rows) + "\n")
    (out / "not-tested.tsv").write_text("".join(f"{cell}\t{ident}\n" for cell, ident in not_tested))
    stamp = "instrument stamp (ours column):\n" + "".join(l for l in tools.column_lines().splitlines(True) if l.startswith("# ours:")).replace("# ours:", "  ours:")
    text = summary(verdicts, not_tested, len(jobs), stamp)
    (out / "summary.txt").write_text(text)
    print(text)
    failed = [r for r in rows if r.split("\t")[3] != "ok"]
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
