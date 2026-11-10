#!/usr/bin/env python3
"""Assemble every `ours` output of the m68k corpus: CPU x level x program, gas as the judge.

m68k_asm_all.py OUT.tsv [--cpus 68000,...] [--levels O1,...] [--jobs N]

One row per cell, never dropped: cpu level prog status folded. status: ok; refused (llrm-cc said so, the reason stays in the row);
cc-fail (llrm-cc failed otherwise); ASM-ERROR (gas refused the assembly: a compiler bug) with gas's first error line. `folded` counts
the `(d,%sp,%Xn)` operands (the premise of the stack-index fold: a cell where it is 0 did not exercise it). Exit 3 on any ASM-ERROR, 2 on
a cc-fail. Environment: LLRM_CC (llrm-cc with the llc beside it), GCC_M68K.
The default CPUs are m68k_corpus_matrix.CPUS, the supported list (test_the_matrix_cpus_are_the_supported_cpus).
"""
import argparse
import os
import re
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import corpus_targets as ct  # noqa: E402
import m68k_corpus_matrix as mx  # noqa: E402
import o0_sweep  # noqa: E402

FOLDED = re.compile(r"\(-?\d+,%sp,%[ad]\d")


def status_of(cc_returncode: int, cc_stderr: str, as_returncode: int | None, as_stderr: str) -> str:
    """The cell's status from llrm-cc's and gas's results (`as_returncode` None when llrm-cc did not produce assembly)."""
    if cc_returncode:
        if re.search(r"refus|not supported yet", cc_stderr):
            lines = [l for l in cc_stderr.splitlines() if l.strip()]
            return "refused " + (lines[-1][:100] if lines else "")
        return "cc-fail " + ((cc_stderr.splitlines() or [""])[-1][:100])
    if as_returncode:
        error = next((l for l in as_stderr.splitlines() if "Error" in l), as_stderr.strip()[:100])
        return "ASM-ERROR " + error.split("Error:", 1)[-1].strip()[:100]
    return "ok"


def exit_status(statuses: list[str]) -> int:
    """3 when gas refused any assembly, 2 when llrm-cc failed for another reason, else 0 (a refusal is named, not a failure)."""
    if any(s.startswith("ASM-ERROR") for s in statuses):
        return 3
    return 2 if any(s.startswith("cc-fail") for s in statuses) else 0


def cell(cc: str, gcc_root: str, cpu: str, level: str, ident: str, path: Path, work: str) -> tuple[str, int]:
    env = {**os.environ, "LD_LIBRARY_PATH": f"{gcc_root}/usr/lib/x86_64-linux-gnu"}
    flags = o0_sweep.flags_of(path.read_text(), f"m68k-{cpu}")
    stem = f"{work}/{ident.replace('/', '_')}.{cpu}.{level}"
    done = subprocess.run([cc, f"-{level}", "-w", "-S", *flags, "-isystem", str(mx.POSIX), str(path), "-o", stem + ".s"],
                          capture_output=True, text=True, env={**env, "LLRM_CC_TARGET": f"m68k-{cpu}"}, timeout=mx.TIMEOUT)
    if done.returncode:
        return status_of(done.returncode, done.stderr, None, ""), 0
    text = mx.relaxing_branches(Path(stem + ".s").read_text())
    Path(stem + ".s").write_text(".text\n" + text)
    gas = subprocess.run([f"{gcc_root}/usr/bin/m68k-linux-gnu-as", "--fatal-warnings", f"-m{cpu}", "-o", stem + ".o", stem + ".s"],
                         capture_output=True, text=True, env=env)
    return status_of(0, "", gas.returncode, gas.stderr), len(FOLDED.findall(text))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("out")
    parser.add_argument("--cpus", default=",".join(mx.CPUS))
    parser.add_argument("--levels", default=",".join(mx.LEVELS))
    parser.add_argument("--jobs", type=int, default=3)
    args = parser.parse_args()
    cc, root = os.environ["LLRM_CC"], os.environ["GCC_M68K"]
    corpus, _ = mx.programs()
    jobs = [(cpu, level, ident, path) for cpu in args.cpus.split(",") for level in args.levels.split(",") for ident, path in corpus
            if ct.applies(f"m68k-{cpu}", path.read_text())]
    statuses = []
    with tempfile.TemporaryDirectory() as work, open(args.out, "w") as out, ThreadPoolExecutor(args.jobs) as pool:
        out.write("cpu\tlevel\tprog\tstatus\tfolded\n")
        for (cpu, level, ident, _), (status, folded) in zip(jobs, pool.map(lambda j: cell(cc, root, j[0], j[1], j[2], j[3], work), jobs)):
            out.write(f"{cpu}\t{level}\t{ident}\t{status}\t{folded}\n")
            statuses.append(status)
    counts = {k: sum(1 for s in statuses if s.split()[0] == k) for k in sorted({s.split()[0] for s in statuses})}
    print(f"{len(jobs)} cells: {counts}")
    return exit_status(statuses)


if __name__ == "__main__":
    sys.exit(main())
