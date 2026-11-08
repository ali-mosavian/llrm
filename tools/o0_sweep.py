#!/usr/bin/env python3
"""A program that does not compile at -O0 on a target it applies to is a tier-1 row.

o0_sweep.py [TARGET...]: compile every tests/run/c program at -O0 (`-c`) for each target `corpus_targets.applies` lets it on, and print per
target: programs (applicable), compiled (the object exists), refused (llrm-cc failed, with the first error line), n/a. Counting
the successes, not the non-failures: a compiler that is not there, or a target that compiled nothing, is red.

Flags from the program's first `// flags:` configuration: its `-D` and `-f` tokens only (`-m*` and `-O*` name a CPU or level the target
row and this sweep already fix). `-I` the POSIX OS layer. LLRM_BIN picks the binaries; JOBS the parallelism (default 4).
Exit 1: a target compiled nothing, or a refusal is not in tools/o0-sweep.baseline (`TARGET PROGRAM`, one per line: a ratchet that only shrinks).
"""
import os
import re
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
import corpus_targets as ct  # noqa: E402

TARGETS = ["x86_16-dos", "x86_32-linux", "x86_64-linux", "arm64-linux", "wasm32", "m68k-68020", "6502-sim65", "z80-sdcc"]
TIMEOUT = 180


def flags_of(source: str) -> list[str]:
    for line in source.splitlines()[:6]:
        if line.startswith("// flags:"):
            first = line.removeprefix("// flags:").split("|")[0].split()
            return [t for t in first if t.startswith(("-D", "-f"))]
    return []


def compiler() -> str:
    given = os.environ.get("LLRM_O0_CC")
    if given:
        return given
    import llrmbin
    return str(llrmbin.bin_dir() / "llrm-cc")


def compile_one(cc: str, target: str, path: Path, workdir: Path) -> tuple[str, str]:
    """("ok", "") when an object was written, else ("refused", first error line)."""
    out = workdir / f"{target}.{path.stem}.o"
    cmd = [cc, "-O0", "-w", "-c", *flags_of(path.read_text()), "-I", str(ROOT / "runtime/shared/posix"), "-o", str(out), str(path)]
    try:
        run = subprocess.run(cmd, capture_output=True, text=True, timeout=TIMEOUT, env={**os.environ, "LLRM_CC_TARGET": target})
    except subprocess.TimeoutExpired:
        return "refused", f"timeout {TIMEOUT}s"
    if run.returncode == 0 and out.exists() and out.stat().st_size > 0:
        out.unlink()
        return "ok", ""
    lines = [l for l in (run.stderr or run.stdout).splitlines() if l.strip()]
    errors = [l for l in lines if re.search(r"error|llrm-|refus|unsupported|Unmapped|panick", l, re.I)]
    return "refused", ((errors or lines or [f"exit {run.returncode}, no object"])[0])[:200]


def sweep(targets: list[str], programs: list[Path], cc: str, jobs: int) -> dict[str, dict]:
    result = {}
    with tempfile.TemporaryDirectory() as tmp, ThreadPoolExecutor(jobs) as pool:
        for target in targets:
            applicable = [p for p in programs if ct.applies(target, p.read_text())]
            rows = list(pool.map(lambda p: (p.stem, *compile_one(cc, target, p, Path(tmp))), applicable))
            result[target] = {
                "programs": len(applicable),
                "na": len(programs) - len(applicable),
                "compiled": sum(1 for _, s, _ in rows if s == "ok"),
                "refused": {n: m for n, s, m in rows if s == "refused"},
            }
    return result


def verdict(result: dict[str, dict], baseline: set[str]) -> list[str]:
    """What is red: a target that compiled nothing, a refusal outside the baseline."""
    bad = []
    for target, row in result.items():
        if row["compiled"] == 0:
            bad.append(f"{target}: compiled 0 of {row['programs']} programs")
        bad += [f"{target} {name}: {msg}" for name, msg in row["refused"].items() if f"{target} {name}" not in baseline]
    return bad


def main(argv: list[str]) -> int:
    targets = argv[1:] or TARGETS
    programs = sorted((ROOT / "tests/run/c").glob("*.c"))
    baseline_file = HERE / "o0-sweep.baseline"
    baseline = {l.strip() for l in baseline_file.read_text().splitlines() if l.strip() and not l.startswith("#")} if baseline_file.exists() else set()
    cc = compiler()
    if not Path(cc).exists():
        print(f"o0_sweep: no compiler at {cc}", file=sys.stderr)
        return 2
    print(f"o0_sweep: {cc} {len(programs)} programs, targets {' '.join(targets)}")
    result = sweep(targets, programs, cc, int(os.environ.get("JOBS", "4")))
    for target, row in result.items():
        print(f"{target}: programs {row['programs']} compiled {row['compiled']} refused {len(row['refused'])} n/a {row['na']}")
        for name, msg in sorted(row["refused"].items()):
            print(f"  REFUSED {name}: {msg}")
    bad = verdict(result, baseline)
    for line in bad:
        print("RED", line)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
