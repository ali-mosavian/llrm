"""
tests/differential/conformance: the Microsoft BASIC compatibility cases. Each
case of a dialect's suite.toml compiles with llrm-qb, links with that dialect's
own runtime library and runs; stdout must equal the case's `expected` (a
literal, or the text of the file it names). `known.toml` maps a failing case to
its issue.

    python3 tools/dosbatch/run_conformance.py [qb45|pds71|vbdos]... [--list-skipped]

Only cases whose `runtime` is `required` and that need one source, no artifact
and no probe run here: the rest are listed as skipped.
"""

from __future__ import annotations

import sys
import argparse
import tomllib
import subprocess
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
llrmbin = dosbatch.llrmbin
from dosbatch import BIN, ROOT, Job  # noqa: E402
from run_tests import first_difference, lines  # noqa: E402

SUITES = ROOT / "tests" / "differential" / "conformance"
PROFILES = {"qb45": dosbatch.QB45_TOOLS, "pds71": dosbatch.PDS71_TOOLS, "vbdos": dosbatch.VBDOS_TOOLS}
UNSUPPORTED = ("companions", "artifacts", "compiler_probe", "expected_diagnostic", "environment")
# BC's switch, llrm-qb's flags
FLAGS = {"/R": ["--array-order", "row-major"], "/Ah": ["--huge-arrays"], "/MBF": ["--mbf"], "/FPa": ["--alternate-math"], "/D": ["-fsanitize=undefined"], "/E": ["--error-lines"]}


def flags(case: dict) -> list[str]:
    return [flag for switch in case["switches"] for flag in FLAGS.get(switch, [])]


def skipped(case: dict) -> str | None:
    if case["runtime"] != "required":
        return f"runtime is {case['runtime']}"
    return next((f"has {key}" for key in UNSUPPORTED if case.get(key)), None)


def compile_one(directory: Path, manifest: dict, case: dict, obj: Path) -> str | None:
    dialect = manifest["dialect"].removesuffix("10")
    command = [str(BIN / "llrm-qb"), str(directory / case["source"]), "--dialect", dialect, "--runtime", dialect,
               *flags(case), "-o", str(obj)]
    done = subprocess.run(command, capture_output=True, text=True, timeout=300)
    if done.returncode != 0 or not obj.exists():
        return "compile: " + (done.stderr or done.stdout).strip()[-400:]
    return None


def expected(directory: Path, case: dict) -> str:
    named = next((one for one in directory.iterdir() if one.name.lower() == case["expected"].lower()), None)
    return named.read_text(encoding="latin1") if named else case["expected"]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("profiles", nargs="*", default=list(PROFILES))
    parser.add_argument("--list-skipped", action="store_true")
    parser.add_argument("--work", type=Path, default=llrmbin.target_dir() / "tests-conformance")
    args = parser.parse_args()
    known = tomllib.loads((SUITES / "known.toml").read_text()) if (SUITES / "known.toml").exists() else {}
    bad = passed = marked = 0
    for profile in args.profiles:
        directory = SUITES / profile
        manifest = tomllib.loads((directory / "suite.toml").read_text())
        cases = []
        for case in manifest["case"]:
            why = skipped(case)
            if why and args.list_skipped:
                print(f"SKIP  {profile}/{case['name']}: {why}")
            elif not why:
                cases.append(case)
        work = args.work / profile
        objs = work.with_name(work.name + "-obj")
        objs.mkdir(parents=True, exist_ok=True)
        stems = {case["name"]: f"C{at:03d}" for at, case in enumerate(cases)}
        with ThreadPoolExecutor() as pool:
            failed = list(pool.map(lambda c: compile_one(directory, manifest, c, objs / f"{stems[c['name']]}.obj"), cases))
        jobs = [Job(stems[c["name"]], "obj", objs / f"{stems[c['name']]}.obj", libs=tuple(rf"V:\LIB\{one}" for one in c.get("link_libraries", [])),
                    args=" ".join(c.get("arguments", [])),
                    library=rf"V:\LIB\{c['runtime_library']}" if c.get("runtime_library") else "") for c, why in zip(cases, failed) if not why]
        ran = dosbatch.run(jobs, work, tools=PROFILES[profile]) if jobs else {}
        for case, problem in zip(cases, failed):
            name = f"{profile}/{case['name']}"
            if not problem:
                result = ran[stems[case["name"]]]
                problem = f"{result.status}: {result.detail}" if result.status != "ok" else first_difference(lines(expected(directory, case)), lines(result.text))
            issue = known.get(name)
            if problem and issue:
                marked += 1
                print(f"KNOWN {name} ({issue}): {problem}")
            elif problem:
                bad += 1
                print(f"FAIL  {name}: {problem}")
            elif issue:
                bad += 1
                print(f"XPASS {name}: passes; remove it from known.toml ({issue})")
            else:
                passed += 1
    print(f"{passed + marked + bad} cases: {passed} pass, {marked} known, {bad} fail")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
