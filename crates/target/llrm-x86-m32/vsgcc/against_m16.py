"""m32 against m16: the same program compiled by llrm for both targets at one -O level, run on a 486, per program the instructions
executed, the memory operands and the 486 clocks of the kernel at each, and their ratio m32/m16. The requirement is that m32 is no
worse than m16: a ratio above 1.00 is a row the requirement fails. See docs/comparison/m32-vs-m16.md.

    uv run --project tools python crates/target/llrm-x86-m32/vsgcc/against_m16.py --run    # measure both, print the tables
    uv run --project tools python crates/target/llrm-x86-m32/vsgcc/against_m16.py          # print them from $VSGCC_WORK's files
"""
from __future__ import annotations

import argparse
import json
import math
import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
WORK = Path(os.environ.get("VSGCC_WORK", Path.home() / "scratch/vsgcc-work"))
LEVELS = ("O1", "O2", "O3", "Os")
COUNTERS = (("instructions", "ins", "ins"), ("memory_operands", "mem", "mem"), ("clocks", "clocks", "clk"))
# What one language's m16 and m32 builds are called in each file: C at m32 is vsgcc's `llrm` variants (-O2 is `llrm`), BASIC's
# are tools/bench's `bq16` and `bq32`. Nib has no m32 build here.
QB = ("bq16", "bq32")


def variant(level: str) -> str:
    """vsgcc's name for llrm at `level`: `llrm` is -O2."""
    return "llrm" if level == "O2" else "llrm" + level


def rows(bench: dict, vsgcc: list[dict], level: str) -> list[dict]:
    """The programs measured at both targets at `level`: `bench` is tools/bench's `--json` (`program/language/level` to counters),
    `vsgcc` the lines of results.jsonl. Each row has `program`, `language` and, per counter, its m16 and m32 value."""
    found = []
    m32 = {r["prog"]: r for r in vsgcc if r["variant"] == variant(level)}
    for key, one in bench.items():
        parts = key.split("/")
        if len(parts) != 3 or parts[2] != level:  # the other keys are the benchmark groups and the references
            continue
        program, language, at = parts
        if language == "c" and program in m32:
            other = {"instructions": m32[program]["ins"], "memory_operands": m32[program]["mem"], "clocks": m32[program]["clocks"]}
            found.append(_row(program, "c", one, other))
        elif language == QB[0] and f"{program}/{QB[1]}/{level}" in bench:
            found.append(_row(program, "qb", one, bench[f"{program}/{QB[1]}/{level}"]))
    # A program with no counter at both targets (a file program BASIC has no flat build of) has no row.
    return sorted((one for one in found if any(one[short] for _, _, short in COUNTERS)), key=lambda one: (one["language"], one["program"]))


def _row(program: str, language: str, m16: dict, m32: dict) -> dict:
    return {
        "program": program,
        "language": language,
        # A counter one side does not measure (BASIC at m32 is counted for instructions only) is none.
        **{short: (m16[name], m32[name]) if name in m16 and name in m32 else None for name, _, short in COUNTERS},
    }


def ratio(row: dict, counter: str) -> float:
    """m32 over m16 of `counter`, a count of none read as one; 0 where it is not measured at both."""
    if row[counter] is None:
        return 0.0
    before, after = row[counter]
    return max(1, after) / max(1, before)


def fails(row: dict) -> bool:
    """Whether the row is worse at m32 as the table prints it: clocks where measured, else instructions, above 1.00 at two places."""
    return round(ratio(row, "clk") or ratio(row, "ins"), 2) > 1.0


def geomean(values: list[float]) -> float:
    return math.exp(sum(math.log(one) for one in values) / len(values))


def table(found: list[dict], level: str) -> list[str]:
    """One level: the rows worst clocks first (instructions where clocks are not measured), then the geomean and the worst row of
    each language and counter."""
    lines = [
        f"-{level}  (m32/m16: above 1.00 fails the requirement)",
        f"{'program':16}{'lang':5}" + "".join(f"{short + ' m16':>11}{short + ' m32':>11}{'ratio':>7}" for _, _, short in COUNTERS),
    ]

    def worst_first(one: dict) -> float:
        return -(ratio(one, "clk") or ratio(one, "ins"))

    for row in sorted(found, key=worst_first):
        cells = "".join(
            f"{row[short][0]:>11}{row[short][1]:>11}{ratio(row, short):>7.2f}" if row[short] else f"{'-':>11}{'-':>11}{'-':>7}"
            for _, _, short in COUNTERS
        )
        lines.append(f"{row['program']:16}{row['language']:5}{cells}{'  <' if fails(row) else ''}")
    for language in sorted({one["language"] for one in found}):
        for _, _, short in COUNTERS:
            group = [one for one in found if one["language"] == language and one[short]]
            if not group:
                continue
            worst = max(group, key=lambda one: ratio(one, short))
            lines.append(
                f"{language:>3} n={len(group):<3} {short:>5}: geomean {geomean([ratio(one, short) for one in group]):.2f}, "
                f"worst {ratio(worst, short):.2f} ({worst['program']}), above 1.00: {sum(round(ratio(one, short), 2) > 1.0 for one in group)}"
            )
    return lines


def measure(bench_file: Path, vsgcc_file: Path) -> None:
    """Both measurements, into `bench_file` and `vsgcc_file`."""
    subprocess.run(
        ["uv", "run", "--project", str(REPO / "tools"), "python", str(REPO / "tools/bench/bench.py"), "--opt", *LEVELS, "--json", str(bench_file)],
        check=False,
        stdout=subprocess.DEVNULL,
    )
    sys.path.insert(0, str(HERE))
    import programs

    env = {**os.environ, "VSGCC_WORK": str(WORK), "VSGCC_ONLY": "llrm"}
    WORK.mkdir(parents=True, exist_ok=True)
    stub = WORK / "stub.o"
    subprocess.run(["gcc", "-m32", "-c", str(HERE / "stub.s"), "-o", str(stub)], check=True)
    emulation = subprocess.run([sys.executable, str(REPO / "tools/linkrecipe.py"), "x86-m32", "ld-emulation"], capture_output=True, text=True, check=True).stdout.strip()
    subprocess.run(["ld", "-m", emulation, "-static", "-e", "0", "-Ttext=0x8000", "-o", str(WORK / "stub.elf"), str(stub)], check=True)
    import hashlib

    (WORK / "stub.elf.src").write_text(hashlib.sha256((HERE / "stub.s").read_bytes()).hexdigest())
    vsgcc_file.write_text("")
    for program in programs.sources():
        subprocess.run(["bash", str(HERE / "build.sh"), program], env=env, check=False)
        for level in LEVELS:
            done = subprocess.run(
                ["uv", "run", "--project", str(REPO / "tools"), "python", str(HERE / "harness.py"), program, variant(level)],
                env=env, capture_output=True, text=True,
            )
            last = done.stdout.strip().splitlines()[-1:] or [""]
            if last[0].startswith("{"):
                with vsgcc_file.open("a") as out:
                    out.write(last[0] + "\n")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--run", action="store_true", help="measure both targets first")
    parser.add_argument("--bench", type=Path, default=WORK / "bench-m16.json", help="tools/bench --json")
    parser.add_argument("--vsgcc", type=Path, default=WORK / "results-m32.jsonl", help="harness.py's lines, llrm variants")
    args = parser.parse_args()
    if args.run:
        measure(args.bench, args.vsgcc)
    bench = json.loads(args.bench.read_text())
    vsgcc = [json.loads(line) for line in args.vsgcc.read_text().splitlines() if line.startswith("{")]
    failed = [(r["prog"], r["variant"]) for r in vsgcc if not r["ok"]]
    assert not failed, f"programs that did not reproduce their output: {failed}"
    for level in LEVELS:
        print("\n".join(table(rows(bench, vsgcc, level), level)) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
