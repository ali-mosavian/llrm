"""GCC's gcc.c-torture/execute through llrm-c: each self-checking program built at -O0, -O2 and -Os, run on the emulator,
and passed if it exits 0.

    uv run --project tools python tools/torture/torture.py [--bits 16|32] [--levels O0,O2,Os] [--stage compile] [--sample N] [names...]

Every program ends in exactly one class: passed; refused by design (`expected.toml` names the refusal and why); a compile
failure; a link failure; a wrong result (it exited non-zero, aborted, or did not finish). The last three are the findings:
none is ever skipped quietly. The corpus is `TORTURE_CORPUS` or ~/work/personal/gcc/gcc/testsuite/gcc.c-torture/execute.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tomllib
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[0] / "dosbatch"))
import dosbatch  # noqa: E402

BIN = dosbatch.BIN
CORPUS = Path(os.environ.get("TORTURE_CORPUS", Path.home() / "work/personal/gcc/gcc/testsuite/gcc.c-torture/execute"))
LEVELS = {"O0": "-O0", "O2": "-O2", "Os": "-Os"}
BATCH = 150
BUDGET_MS = 15_000
COMPILE_SECONDS = 60


def programs(names: list[str]) -> list[Path]:
    found = sorted(CORPUS.glob("*.c"))
    if names:
        found = [one for one in found if one.stem in names or one.name in names]
    return found


def cause(text: str) -> str:
    """What a compiler's complaint says with its place and its names taken out: the key failures are counted by."""
    lines = [one.strip() for one in text.strip().splitlines() if one.strip()]
    # wccq's own: the first of its errors says it
    line = next((one for one in lines if "Error!" in one), lines[0] if lines else "")
    line = re.sub(r"^.*Error!\s*E\d+:\s*", "wccq: ", line)
    line = re.sub(r"^llrm-c:\s*", "", line)
    line = re.sub(r"^\S+\.c:\d+(:\d+)?:\s*", "", line)
    line = re.sub(r"'[^']*'", "'_'", line)
    line = re.sub(r"\b(n)?\d+\b", lambda found: "nN" if found.group(1) else "N", line)
    return line[:160]


def expected() -> list[dict]:
    with open(HERE / "expected.toml", "rb") as handle:
        return tomllib.load(handle).get("refused", [])


def differences() -> dict[str, str]:
    """The programs that run and exit non-zero by design, each with its reason: `[[differs]]` in expected.toml."""
    with open(HERE / "expected.toml", "rb") as handle:
        return {one["program"]: one["reason"] for one in tomllib.load(handle).get("differs", [])}


def refusal(text: str, rules: list[dict]) -> str | None:
    for rule in rules:
        if re.search(rule["match"], text):
            return rule["reason"]
    return None


def spellings(symbol: str) -> list[str]:
    """What a linker's undefined symbol is called in C: the Watcom convention adds a trailing underscore (`sprintf_`), cdecl a leading
    one (`_sprintf`, `___builtin_ffs`), and the target decides which."""
    return list(dict.fromkeys([symbol.rstrip("_"), symbol[1:] if symbol.startswith("_") else symbol, symbol]))


def build(source: Path, level: str, target: str, work: Path, support: Path, stem: str, rules: list[dict]) -> tuple[str, str, Path | None]:
    """(class, why, exe): class is built, refused, compile or link."""
    obj = work / f"{stem}.obj"
    # A program that says (dg-require-effective-target) it needs a 32-bit int or pointer is not run where they are 16 bits.
    if dosbatch.target_bits(target) == 16:
        needs = re.search(r"dg-require-effective-target\s+(int32plus|size32plus|ptr32plus)", source.read_text(errors="replace"))
        if needs:
            return "refused", f"the program requires {needs.group(1)} (dg-require-effective-target): the real-mode target's int is 16 bits", None
    try:
        done = subprocess.run([str(BIN / "llrm-c"), str(source), "-I", str(HERE / "include"), "-I", str(dosbatch.c_include(target, work)), dosbatch.m_flag(target), LEVELS[level], "-o", str(obj)],
                              capture_output=True, text=True, timeout=COMPILE_SECONDS)
    except subprocess.TimeoutExpired:
        return "compile", f"did not finish in {COMPILE_SECONDS} s", None
    if done.returncode != 0 or not obj.exists():
        found = cause(done.stderr or done.stdout) or f"exit {done.returncode}"
        why = refusal(found, rules)
        return ("refused", why, None) if why else ("compile", found, None)
    exe = work / f"{stem}.exe"
    try:
        loaders = dosbatch.link_target(target, obj, exe, work, objects_after=(support,))
        dosbatch.check_loads(exe)
    except (dosbatch.BuildError, dosbatch.TooBig) as error:
        text = str(error)
        missing = re.search(r"undefined symbol (\S+)", text)
        named = [f"undefined symbol {one}" for one in spellings(missing.group(1))] if missing else [text]
        why = next(filter(None, (refusal(one, rules) for one in named)), None)
        return ("refused", why, None) if why else ("link", named[0] if missing else cause(text), None)
    return "built", "", exe


def launched(jobs: list, work: Path) -> dict:
    """Every job's result. A launch that lost a job (a program that took the emulator with it) is run again in halves until the
    one is alone, and that one is a wrong result of its own kind: the emulator did not come back."""
    try:
        return dosbatch.run(jobs, work, budget_ms=BUDGET_MS)
    except RuntimeError as error:
        if len(jobs) == 1:
            return {jobs[0].stem: dosbatch.Result("lost", detail="the emulator did not report the program's end")}
        middle = len(jobs) // 2
        return {**launched(jobs[:middle], work), **launched(jobs[middle:], work)}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("names", nargs="*")
    parser.add_argument("--bits", type=int, choices=[16, 32], default=32, help="the target's int width")
    parser.add_argument("--levels", default="O0,O2,Os")
    parser.add_argument("--stage", choices=["compile", "run"], default="run")
    parser.add_argument("--sample", type=int, default=0, help="the first N programs only")
    parser.add_argument("--gate", action="store_true", help="the fixed sample of sample.txt, all levels; exits 1 on any build, link or wrong result that is not named")
    parser.add_argument("--work", type=Path, default=Path.home() / "scratch/torture-work")
    parser.add_argument("--out", type=Path, help="write each program's result here as JSON")
    args = parser.parse_args()
    target = dosbatch.linkrecipe.named(args.bits)
    work = args.work
    work.mkdir(parents=True, exist_ok=True)
    rules = expected()
    names_wanted = args.names
    if args.gate:
        names_wanted = [line.strip() for line in (HERE / "sample.txt").read_text().splitlines() if line.strip() and not line.startswith("#")]
    chosen = programs(names_wanted)[: args.sample or None]
    if not chosen:
        print(f"no programs in {CORPUS} (TORTURE_CORPUS names the gcc.c-torture/execute directory)")
        return 2
    support_obj = work / "libc.obj"
    done = subprocess.run([str(BIN / "llrm-c"), str(HERE / "libc.c"), "-I", str(HERE / "include"), "-I", str(dosbatch.c_include(target, work)), dosbatch.m_flag(target), "-O2", "-o", str(support_obj)], capture_output=True, text=True)
    if done.returncode != 0:
        print("libc.c: " + done.stderr)
        return 2
    cases = [(source, level) for source in chosen for level in args.levels.split(",")]
    names = {case: f"T{at:04d}" for at, case in enumerate(cases)}
    result: dict[tuple[Path, str], tuple[str, str]] = {}
    exes: dict[tuple[Path, str], Path] = {}

    def one(case):
        source, level = case
        return case, build(source, level, target, work, support_obj, names[case], rules)

    with ThreadPoolExecutor(max_workers=os.cpu_count() or 4) as pool:
        for case, (kind, why, exe) in pool.map(one, cases):
            if kind == "built":
                exes[case] = exe
                result[case] = ("built", "")
            else:
                result[case] = (kind, why)
    if args.stage == "run":
        order = list(exes)
        for start in range(0, len(order), BATCH):
            group = order[start : start + BATCH]
            loader = dosbatch.target_link(target).get("loader")
            files = (Path(loader.replace("{ow}", str(dosbatch.ow_root()))),) if loader else ()
            jobs = [dosbatch.Job(names[case], "exe", exes[case], files=files) for case in group]
            ran = launched(jobs, work / "run")
            for case in group:
                outcome = ran[names[case]]
                if outcome.status != "ok":
                    result[case] = ("wrong", f"{outcome.status}: {outcome.detail[:80]}")
                elif outcome.exit_code:
                    result[case] = ("wrong", "aborted" if outcome.exit_code == 134 else f"exit code {outcome.exit_code}")
                else:
                    result[case] = ("pass", "")
            print(f"ran {min(start + BATCH, len(order))}/{len(order)}", file=sys.stderr)
    by_design = differences()
    for case, (kind, why) in list(result.items()):
        if kind == "wrong" and case[0].stem in by_design:
            result[case] = ("differs", f"{why}: {by_design[case[0].stem]}")
    summary(result, args)
    if args.gate:
        failed = sorted(f"{case[0].stem}@{case[1]}" for case, (kind, _) in result.items() if kind in ("compile", "link", "wrong"))
        print("GATE: " + ("ok" if not failed else "FAILED: " + ", ".join(failed)))
        if failed:
            return 1
    if args.out:
        args.out.write_text(json.dumps({f"{case[0].stem}@{case[1]}": list(value) for case, value in result.items()}, indent=1))
    return 0


def summary(result, args) -> None:
    total = Counter(kind for kind, _ in result.values())
    print(f"{len(result)} builds: " + ", ".join(f"{count} {kind}" for kind, count in sorted(total.items())))
    for kind in ("refused", "differs", "compile", "link", "wrong"):
        causes: dict[str, list[str]] = defaultdict(list)
        for (source, level), (found, why) in result.items():
            if found == kind:
                causes[why].append(f"{source.stem}@{level}")
        if not causes:
            continue
        print(f"\n== {kind}: {sum(len(v) for v in causes.values())}")
        for why, cases in sorted(causes.items(), key=lambda item: -len(item[1]))[:40]:
            print(f"{len(cases):5d}  {why}  [{', '.join(cases[:3])}]")


if __name__ == "__main__":
    sys.exit(main())
