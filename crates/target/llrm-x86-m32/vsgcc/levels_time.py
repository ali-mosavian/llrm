"""Compile time of every -O level of llrm, gcc and clang on the same C programs: user instructions retired, median of
RUNS runs, one process each, source to object (`-m32`, no assembler or linker step of ours).

    uv run --project tools python crates/target/llrm-x86-m32/vsgcc/levels_time.py [--runs 3] [--extra DIR ...]

Writes $VSGCC_WORK/levels_time.json and prints the tables. `--extra DIR` adds the `*.c` under DIR that gcc and clang both
accept; the x_* kernels in `kernels/` are always in. What each side times, as `perf stat -e instructions:u` counts it (children included):
  llrm-c   C front end (its preprocessor included), HIR, MIR passes, instruction selection, register allocation, OMF object
           writer; one process.
  gcc -c   the driver, cc1 (preprocessor and compiler) and `as`; three processes.
  clang -c the driver and the cc1 process with its integrated assembler.
`empty.c` is compiled the same way at every level: what a compiler spends on nothing, to read the rest against.
"""
import argparse, json, math, os, statistics, subprocess, sys, tempfile
from pathlib import Path

R = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(R / "tools"))
import llrmbin

O = Path(os.environ.get("VSGCC_WORK", Path.home() / "scratch/vsgcc-work"))
SKIP = {"readme.md", "parity", "huge", "textfill", "grep"}
LEVELS = ["O0", "O1", "O2", "O3", "Os"]
FLAGS = ["-m32", "-march=i486", "-fno-pic", "-fno-stack-protector", "-fcf-protection=none", "-Dfar="]


def command(compiler: str, level: str, source: Path) -> list[str]:
    if compiler == "llrm":
        return [str(llrmbin.bin_dir() / "llrm-c"), "-m32", "-march=i486", f"-{level}", "-o", "/dev/null", str(source)]
    return [compiler, *FLAGS, f"-{level}", "-c", "-o", "/dev/null", str(source)]


def instructions(cmd: list[str]) -> int:
    with tempfile.NamedTemporaryFile("r") as out:
        subprocess.run(["perf", "stat", "-x,", "-e", "instructions:u", "-o", out.name, *cmd], check=True, capture_output=True)
        line = [l for l in out.read().splitlines() if "instructions" in l][0]
    return int(line.split(",")[0])


def measure(cmd: list[str], runs: int) -> tuple[int, float]:
    counts = [instructions(cmd) for _ in range(runs)]
    median = statistics.median(counts)
    return int(median), (max(counts) - min(counts)) / median


def accepted(source: Path) -> bool:
    return all(subprocess.run(command(c, "O0", source), capture_output=True).returncode == 0 for c in ("gcc", "clang"))


def geomean(values: list[float]) -> float:
    return math.exp(sum(math.log(v) for v in values) / len(values))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--extra", nargs="*", default=[])
    args = parser.parse_args()
    sources = {p: R / "bench" / p / f"{p}.c" for p in sorted(x.name for x in (R / "bench").iterdir()) if p not in SKIP}
    kernels = Path(__file__).resolve().parent / "kernels"
    for directory in ([kernels] if kernels.is_dir() else []) + [Path(d) for d in args.extra]:
        for source in sorted(Path(directory).glob("**/*.c")):
            if accepted(source):
                sources[source.stem if source.stem.startswith("x_") else f"{Path(directory).name}/{source.stem}"] = source
    with tempfile.TemporaryDirectory() as scratch:
        empty = Path(scratch) / "empty.c"
        empty.write_text("")
        sources["empty"] = empty
        results: dict[str, dict[str, list]] = {}
        for name, source in sources.items():
            for compiler in ("llrm", "gcc", "clang"):
                for level in LEVELS + (["Omax"] if compiler == "llrm" else []):
                    count, spread = measure(command(compiler, level, source), args.runs)
                    results.setdefault(name, {})[f"{compiler} {level}"] = [count, spread]
    json.dump(results, open(O / "levels_time.json", "w"), indent=1)
    report(results, args.runs)


def report(results: dict[str, dict[str, list]], runs: int) -> None:
    programs = [p for p in results if p != "empty"]
    print(f"{len(programs)} programs, instructions:u, median of {runs}; worst spread (max-min)/median {max(v[1] for r in results.values() for v in r.values()):.1%}")
    print("startup (empty.c), instructions: " + ", ".join(f"{k} {v[0]:,}" for k, v in results["empty"].items() if k.endswith(" O0") or k.endswith(" O2")))
    print(f"{'level':8} {'llrm/gcc':>22} {'llrm/clang':>22}")
    for level in LEVELS + ["Omax"]:
        against = "O3" if level == "Omax" else level
        cells = []
        for other in ("gcc", "clang"):
            ratios = {p: results[p][f"llrm {level}"][0] / results[p][f"{other} {against}"][0] for p in programs}
            worst = max(ratios, key=ratios.get)
            cells.append(f"{geomean(list(ratios.values())):6.2f}x  worst {ratios[worst]:6.2f}x {worst}")
        print(f"{level:8} {cells[0]:>22} {cells[1]:>22}" + ("  (against O3)" if level == "Omax" else ""))


if __name__ == "__main__":
    main()
