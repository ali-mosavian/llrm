#!/usr/bin/env python3
"""A program that does not compile on a target it applies to, at any level, is a tier-1 row.

o0_sweep.py [--levels O0,O1,O2,Os,O3] [TARGET...]: compile every tests/run/c program (`-c`) for each target `corpus_targets.applies` lets it on, at each
level (default: all five), and print per target and level: programs (applicable), compiled (the object exists), refused (llrm-cc failed, with the
first error line), n/a (the program's `// targets:` header excludes the target: not run, not a refusal; a refusal is a program that applies and
that the compiler could not build). Counting the successes, not the non-failures: a compiler that is not there, or a target and level that compiled
nothing, is red. `Legalize Unsupported { op, ty, what }` refusals are listed as op/type/what.

Flags from the program's first `// flags:` configuration: its `-D` and `-f` tokens, and at x86 targets its feature `-m` tokens (`-mlong-double-80`), not `-O*` or the
CPU/ABI/model ones (`-march`, `-mabi`, `-mcmodel`, `-m32`), which the target row and this sweep fix. `-I` the POSIX OS layer. LLRM_BIN picks the binaries; JOBS the parallelism (default 4).
Exit 1: a target with no applicable program (a broken `applies` filter) or none compiled, or a refusal not in tools/o0-sweep.baseline.
The baseline is a ratchet that only shrinks, one entry per line: `TARGET LEVELS PROGRAM | OWNER | EXIT YYYY-MM-DD | ERROR LINE`, LEVELS a comma list
(`O1,O2`) or `all`. An entry missing a field, past its exit date, that now compiles, or that names a run that did not happen (a program the target
does not apply to, a level not swept in this run is skipped) is red: a list of known refusals must not hide a tier-1 row.
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
LEVELS = ["O0", "O1", "O2", "Os", "O3"]
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
            # -f tokens (-fexcess-precision, -fno-inline=NAME) are llrm-cc's x86 options: the other backends refuse an unknown one, so a non-x86 sweep would list its own flags as refusals
            kept = [t for t in first if t.startswith(("-D", "-W")) or (feature and t.startswith("-f")) or (feature and t.startswith("-m") and not t.startswith(CPU_OR_ABI) and t not in ("-m16", "-m32", "-m64"))]
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


def compile_one(cc: str, target: str, path: Path, workdir: Path, level: str = "O0") -> tuple[str, str]:
    """("ok", "") when an object was written, ("timeout", ...) when the compiler ran out of time (a cell not tested, under host load: not a
    refusal), else ("refused", first error line)."""
    out = workdir / f"{target}.{level}.{path.stem}.o"
    # The DOS rows find their own llrm_os.h (Watcom's); the POSIX layer's would shadow it.
    posix = [] if target.endswith("-wat") or target.endswith("-dos") else ["-I", str(ROOT / "runtime/shared/posix")]
    cmd = [cc, f"-{level}", "-w", "-c", *flags_of(path.read_text(), target), *posix, "-o", str(out), str(path)]
    try:
        run = subprocess.run(cmd, capture_output=True, text=True, timeout=TIMEOUT, env={**os.environ, **cc_env(), "LLRM_CC_TARGET": target})
    except subprocess.TimeoutExpired:
        return "timeout", f"timeout {TIMEOUT}s"
    if run.returncode == 0 and out.exists() and out.stat().st_size > 0:
        out.unlink()
        return "ok", ""
    lines = [l for l in (run.stderr or run.stdout).splitlines() if l.strip()]
    errors = [l for l in lines if re.search(r"error|llrm-|refus|unsupported|Unmapped|panick", l, re.I)]
    return "refused", ((errors or lines or [f"exit {run.returncode}, no object"])[0])[:200]


def sweep(targets: list[str], programs: list[Path], cc: str, jobs: int, levels: list[str] | None = None) -> dict[str, dict[str, dict]]:
    """result[target][level] = programs / n/a / compiled / refused {program: first error line}."""
    result: dict[str, dict[str, dict]] = {}
    with tempfile.TemporaryDirectory() as tmp, ThreadPoolExecutor(jobs) as pool:
        for target in targets:
            applicable = [p for p in programs if ct.applies(target, p.read_text())]
            for level in levels or ["O0"]:
                rows = list(pool.map(lambda p: (p.stem, *compile_one(cc, target, p, Path(tmp), level)), applicable))
                result.setdefault(target, {})[level] = {
                    "programs": len(applicable),
                    "na": len(programs) - len(applicable),
                    "compiled": sum(1 for _, s, _ in rows if s == "ok"),
                    "refused": {n: m for n, s, m in rows if s == "refused"},
                    "timeouts": sorted(n for n, s, _ in rows if s == "timeout"),
                    "applicable": sorted(p.stem for p in applicable),
                }
    return result


LEGALIZE = re.compile(r"Unsupported \{ op: (?P<op>\w+), ty: (?:Simple\()?(?P<ty>\w+)\)?, what: \\?\"(?P<what>[^\"\\]+)")


def explain(message: str) -> str:
    """`op ty what` of a `Legalize Unsupported` refusal; any other refusal keeps its first error line."""
    m = LEGALIZE.search(message)
    return f"Legalize Unsupported {m['op']} {m['ty']} ({m['what']})" if m else message


def parse_baseline(text: str, today: str) -> tuple[set[tuple[str, str, str]], list[str]]:
    """({(target, level, program)} that are excused, the entries that are malformed or past their exit date)."""
    keys, bad = set(), []
    for line in text.splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        fields = [f.strip() for f in line.split("|")]
        date = fields[2].removeprefix("EXIT ").strip() if len(fields) == 4 else ""
        head = fields[0].split() if fields else []
        levels = (LEVELS if head[1:2] == ["all"] else head[1].split(",")) if len(head) == 3 else []
        if len(fields) != 4 or not all(fields) or len(head) != 3 or not set(levels) <= set(LEVELS) or not re.fullmatch(r"\d{4}-\d\d-\d\d", date):
            bad.append(f"baseline entry needs 'TARGET LEVELS PROGRAM | OWNER | EXIT YYYY-MM-DD | ERROR' (LEVELS O0,O1,O2,Os,O3 or all): {line}")
        elif date < today:
            bad.append(f"baseline entry past its exit date {date}: {fields[0]}")
        else:
            keys |= {(head[0], level, head[2]) for level in levels}
    return keys, bad


def verdict(result: dict[str, dict[str, dict]], baseline: set[tuple[str, str, str]]) -> list[str]:
    """What is red: a target and level with no applicable program or that compiled nothing, a refusal outside the baseline, a baseline row that
    compiled now or never ran (a program the target does not apply to). A level not swept in this run leaves its rows alone."""
    bad = []
    for target, levels in result.items():
        for level, row in levels.items():
            at = f"{target} {level}"
            if row["programs"] == 0:
                bad.append(f"{at}: 0 applicable programs (a broken applies filter?)")
            elif row["compiled"] == 0:
                bad.append(f"{at}: compiled 0 of {row['programs']} programs")
            bad += [f"{at} {name}: {explain(msg)}" for name, msg in row["refused"].items() if (target, level, name) not in baseline]
            bad += [f"{at} {name}: timed out, not tested (not a refusal): rerun it" for name in row.get("timeouts", [])]
            for t, l, name in sorted(baseline):
                if (t, l) != (target, level):
                    continue
                if name not in row["applicable"]:
                    bad.append(f"{at} {name}: in the baseline but never ran (not an applicable program): remove the row")
                elif name not in row["refused"] and name not in row.get("timeouts", []):
                    bad.append(f"{at} {name}: in the baseline but compiled: remove the row")
    return bad


def candidate_rows(result: dict[str, dict[str, dict]]) -> list[str]:
    """Baseline lines for every refusal, the levels of one (target, program, error) merged: `TARGET O1,O2 PROGRAM | OWNER? | EXIT? | ERROR`.
    OWNER? and EXIT? are placeholders a person fills; parse_baseline rejects a line that keeps EXIT? (not a date), so an unfilled row is red."""
    merged: dict[tuple[str, str, str], list[str]] = {}
    for target, by_level in result.items():
        for level, row in by_level.items():
            for name, msg in row["refused"].items():
                merged.setdefault((target, name, explain(msg)), []).append(level)
    return [f"{t} {','.join(sorted(ls, key=LEVELS.index))} {n} | OWNER? | EXIT? | {m}" for (t, n, m), ls in sorted(merged.items())]


def main(argv: list[str]) -> int:
    args, levels = argv[1:], LEVELS
    if args[:1] == ["--levels"]:
        levels, args = args[1].split(","), args[2:]
    targets = args or TARGETS
    programs = sorted((ROOT / "tests/run/c").glob("*.c"))
    baseline_file = HERE / "o0-sweep.baseline"
    import datetime
    baseline, baseline_bad = parse_baseline(baseline_file.read_text() if baseline_file.exists() else "", datetime.date.today().isoformat())
    cc = compiler()
    if not Path(cc).exists():
        print(f"o0_sweep: no compiler at {cc}", file=sys.stderr)
        return 2
    print(f"o0_sweep: {cc} {len(programs)} programs, targets {' '.join(targets)}, levels {' '.join(levels)}")
    result = sweep(targets, programs, cc, int(os.environ.get("JOBS", "4")), levels)
    for target, by_level in result.items():
        for level, row in by_level.items():
            print(f"{target} {level}: programs {row['programs']} compiled {row['compiled']} refused {len(row['refused'])} timeout {len(row['timeouts'])} n/a {row['na']}")
            for name, msg in sorted(row["refused"].items()):
                print(f"  REFUSED {name}: {explain(msg)}")
            for name in row["timeouts"]:
                print(f"  TIMEOUT {name}: not tested")
    bad = baseline_bad + verdict(result, baseline)
    if os.environ.get("O0_SWEEP_ROWS"):
        print("candidate baseline rows:", *candidate_rows(result), sep="\n")
    for line in bad:
        print("RED", line)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
