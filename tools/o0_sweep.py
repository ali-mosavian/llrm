#!/usr/bin/env python3
"""A program that does not compile at -O0 on a target it applies to is a tier-1 row.

o0_sweep.py [TARGET...]: compile every tests/run/c program at -O0 (`-c`) for each target `corpus_targets.applies` lets it on, and print per
target: programs (applicable), compiled (the object exists), refused (llrm-cc failed, with the first error line), n/a. Counting
the successes, not the non-failures: a compiler that is not there, or a target that compiled nothing, is red.

Flags from the program's first `// flags:` configuration: its `-D` and `-f` tokens, and at x86 targets its feature `-m` tokens (`-mlong-double-80`), not `-O*` or the
CPU/ABI/model ones (`-march`, `-mabi`, `-mcmodel`, `-m32`), which the target row and this sweep fix. `-I` the POSIX OS layer. LLRM_BIN picks the binaries; JOBS the parallelism (default 4).
Exit 1: a target with no applicable program (a broken `applies` filter) or none compiled, or a refusal not in tools/o0-sweep.baseline.
The baseline is a ratchet that only shrinks, one entry per line: `TARGET PROGRAM | OWNER | EXIT YYYY-MM-DD | ERROR LINE`. An entry missing a field, or past its
exit date, is red: a list of known refusals must not hide a tier-1 row.
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

TARGETS = ["x86_16-wat", "x86_32-wat", "x86_32-linux", "x86_64-linux", "arm64-linux", "wasm32", "m68k-68000", "m68k-68020", "6502-sim65", "z80-sdcc"]
TIMEOUT = 180
CPU_OR_ABI = ("-march", "-mcpu", "-mtune", "-mabi", "-mcmodel")  # named by the target row


def flags_of(source: str, target: str = "x86") -> list[str]:
    """The first `// flags:` configuration's feature flags, then the target family's `// flags-FAMILY:` line (corpus_targets.flags_for:
    the same source as the executed sweeps; dropping `-mcmodel=medium` made memcpy_runtime overflow a pc-relative displacement on m68k)."""
    kept: list[str] = []
    for line in source.splitlines()[:6]:
        if line.startswith("// flags:"):
            first = line.removeprefix("// flags:").split("|")[0].split()
            feature = target.startswith("x86")  # -mlong-double-80 and the like are x87 options; other drivers reject them
            kept = [t for t in first if t.startswith(("-D", "-f", "-W")) or (feature and t.startswith("-m") and not t.startswith(CPU_OR_ABI) and t not in ("-m16", "-m32", "-m64"))]
            break
    return [*kept, *ct.flags_for(target.split("-")[0], source)]


def compiler() -> str:
    given = os.environ.get("LLRM_O0_CC")
    if given:
        return given
    import llrmbin
    return str(llrmbin.bin_dir() / "llrm-cc")


def cc_env() -> dict[str, str]:
    """What llrm-cc needs to find its clang for the x86_16 rows (tools/llrmbin.cc_environment): without it every x86_16-dos program is refused."""
    import llrmbin
    return llrmbin.cc_environment()


def compile_one(cc: str, target: str, path: Path, workdir: Path) -> tuple[str, str]:
    """("ok", "") when an object was written, else ("refused", first error line)."""
    out = workdir / f"{target}.{path.stem}.o"
    # The DOS rows find their own llrm_os.h (Watcom's); the POSIX layer's would shadow it.
    posix = [] if target.endswith("-wat") or target.endswith("-dos") else ["-I", str(ROOT / "runtime/shared/posix")]
    cmd = [cc, "-O0", "-w", "-c", *flags_of(path.read_text(), target), *posix, "-o", str(out), str(path)]
    try:
        run = subprocess.run(cmd, capture_output=True, text=True, timeout=TIMEOUT, env={**os.environ, **cc_env(), "LLRM_CC_TARGET": target})
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


def parse_baseline(text: str, today: str) -> tuple[set[str], list[str]]:
    """(the TARGET PROGRAM keys that are excused, the entries that are malformed or past their exit date)."""
    keys, bad = set(), []
    for line in text.splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        fields = [f.strip() for f in line.split("|")]
        date = fields[2].removeprefix("EXIT ").strip() if len(fields) == 4 else ""
        if len(fields) != 4 or not all(fields) or not re.fullmatch(r"\d{4}-\d\d-\d\d", date):
            bad.append(f"baseline entry needs 'TARGET PROGRAM | OWNER | EXIT YYYY-MM-DD | ERROR': {line}")
        elif date < today:
            bad.append(f"baseline entry past its exit date {date}: {fields[0]}")
        else:
            keys.add(fields[0])
    return keys, bad


def verdict(result: dict[str, dict], baseline: set[str]) -> list[str]:
    """What is red: a target with no applicable program, one that compiled nothing, a refusal outside the baseline."""
    bad = []
    for target, row in result.items():
        if row["programs"] == 0:
            bad.append(f"{target}: 0 applicable programs (a broken applies filter?)")
        elif row["compiled"] == 0:
            bad.append(f"{target}: compiled 0 of {row['programs']} programs")
        bad += [f"{target} {name}: {msg}" for name, msg in row["refused"].items() if f"{target} {name}" not in baseline]
    return bad


def main(argv: list[str]) -> int:
    targets = argv[1:] or TARGETS
    programs = sorted((ROOT / "tests/run/c").glob("*.c"))
    baseline_file = HERE / "o0-sweep.baseline"
    import datetime
    baseline, baseline_bad = parse_baseline(baseline_file.read_text() if baseline_file.exists() else "", datetime.date.today().isoformat())
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
    bad = baseline_bad + verdict(result, baseline)
    for line in bad:
        print("RED", line)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
