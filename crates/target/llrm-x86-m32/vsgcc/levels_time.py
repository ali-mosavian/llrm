"""Compile time of every -O level of llrm, gcc and clang on the same C programs: user instructions retired, median of
RUNS runs, one process each, source to object (`-m32`, no assembler or linker step of ours).

    uv run --project tools python crates/target/llrm-x86-m32/vsgcc/levels_time.py [--runs 3] [--extra DIR ...]

Writes $VSGCC_WORK/levels_time.json and prints the tables. `--extra DIR` adds the `*.c` under DIR that gcc and clang both
accept; the x_* kernels in `kernels/` are always in. What each side times, as `perf stat -e instructions:u` counts it (children included):
  llrm-cc  the driver, clang's front end (the preprocessor included), then llrm-llc: MIR passes, instruction selection, register
           allocation, OMF object writer; three processes.
  gcc -c   the driver, cc1 (preprocessor and compiler) and `as`; three processes.
  clang -c the driver and the cc1 process with its integrated assembler.
`empty.c` is compiled the same way at every level: what a compiler spends on nothing, to read the rest against.
"""
import argparse, functools, json, math, os, statistics, subprocess, sys, tempfile
from pathlib import Path

R = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(R / "tools"))
import llrmbin
import pin_clang
import benchlist
import programs

O = Path(os.environ.get("VSGCC_WORK", Path.home() / "scratch/vsgcc-work"))
SKIP = {"readme.md", "parity", "huge", "textfill", "grep"}
LEVELS = ["O0", "O1", "O2", "O3", "Os"]
FLAGS = ["-m32", "-march=i486", "-fno-pic", "-fno-stack-protector", "-fcf-protection=none", "-Dfar="]


# Address space layout randomisation moves what a count depends on: a Python `sum(range(...))` read 12,969 or 13,381 Minstr (50
# runs: 7 and 43; 50 of 50 the lower with it off), and `lir peephole` read two counts by where mimalloc's pages fell. A measured
# command runs with the layout fixed, so one run and the next, base and head, are given the same addresses.
UNRANDOMIZED = ["setarch", "-R"]


def command(compiler: str, level: str, source: Path, bits: int = 32) -> list[str]:
    if compiler == "llrm":
        return [str(llrmbin.pinned(llrmbin.bin_dir() / "llrm-cc")), *llrmbin.c_args(bits), f"-{level}", "-c", "-o", llrmbin.OBJECT, str(source)]
    if compiler == "clang":
        compiler = pin_clang.path()  # not PATH's: the distro's 20.1.8 is not the pin
    return [compiler, *FLAGS, f"-{level}", "-c", "-o", "/dev/null", str(source)]


SLOWDOWN = 60  # CPU seconds of a run under callgrind per native CPU second (about 50x measured on a compile)


@functools.cache
def perf_counts() -> bool:
    """Whether `perf stat` reads an instruction count here (a hosted VM exposes no counters: it prints `<not supported>`)."""
    return subprocess.run("perf stat -x, -e instructions:u true 2>&1 | grep -Eq '^[0-9]+,'", shell=True).returncode == 0


def backend() -> str:
    """What counts instructions: LLRM_COUNTER=perf, or callgrind (valgrind's executed-instruction count: exact, and the same on every
    host), else perf where it counts and callgrind where it does not. A count of one backend is not comparable with another's (callgrind
    read 102.67 M of a compile perf read 103.84 M of), so a stored measurement names its backend."""
    chosen = os.environ.get("LLRM_COUNTER") or "auto"
    if chosen not in ("auto", "perf", "callgrind"):
        raise SystemExit(f"LLRM_COUNTER is {chosen!r}: perf, callgrind or auto")
    return chosen if chosen != "auto" else "perf" if perf_counts() else "callgrind"


def counted(cmd: list[str], env: dict | None = None, preexec=None, timeout: float | None = None, cwd=None) -> tuple[int, int, subprocess.CompletedProcess]:
    """(user instructions, task-clock ns, the finished process) of one run of `cmd`, with the address space fixed. The process's
    returncode and stderr are the command's own. Raises RuntimeError where the backend counted nothing."""
    import resource

    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    with tempfile.NamedTemporaryFile("r") as out, llrmbin.with_object(cmd) as cmd:
        if backend() == "perf":
            wrapped = ["perf", "stat", "-x,", "-e", "instructions:u,task-clock", "-o", out.name, "--"]
        else:  # the tool's own messages go to a file, so the command's stderr is its own
            wrapped = ["valgrind", "--tool=callgrind", "--callgrind-out-file=/dev/null", f"--log-file={out.name}"]
        done = subprocess.run([*UNRANDOMIZED, *wrapped, *cmd], capture_output=True, text=True, env=env, preexec_fn=preexec, timeout=timeout, cwd=cwd)
        text = out.read()
    if backend() == "perf":
        found = {l.split(",")[2]: int(float(l.split(",")[0])) for l in text.splitlines() if l and not l.startswith("#") and l.split(",")[0].replace(".", "").isdigit()}
        count, clock = found.get("instructions:u", 0), found.get("task-clock", 0)
    else:
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        collected = [l.split(":")[-1].strip().replace(",", "") for l in text.splitlines() if "Collected" in l]
        count = int(collected[0]) if collected and collected[0].isdigit() else 0
        clock = int((after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime) * 1e9)
    if count <= 0:
        raise RuntimeError(f"{backend()} counted nothing: {(text or done.stderr)[-200:]!r}")
    return count, clock, done


def instructions(cmd: list[str]) -> int:
    count, _, done = counted(cmd)
    if done.returncode:
        raise subprocess.CalledProcessError(done.returncode, cmd, done.stdout, done.stderr)
    return count


def measure(cmd: list[str], runs: int) -> tuple[int, float]:
    counts = [instructions(cmd) for _ in range(runs)]
    median = statistics.median(counts)
    return int(median), (max(counts) - min(counts)) / median


def accepted(source: Path) -> bool:
    return all(subprocess.run(command(c, "O0", source), capture_output=True).returncode == 0 for c in ("gcc", "clang"))


def geomean(values: list[float]) -> float:
    return math.exp(sum(math.log(v) for v in values) / len(values))


def setup() -> None:
    """What a user's compile does: no check of each pass and phase (the cargo environment of a test run would turn it on), and llrm-cc
    given its Open Watcom tree and pinned clang (without them the -O0 bench/bintree compile exited 1)."""
    os.environ["LLRM_VERIFY"] = "0"
    os.environ.update(llrmbin.cc_environment())


def bench_sources() -> dict[str, Path]:
    """The bench programs gcc and clang also build: not the ones that include bench_io.h (`programs.sources`' own filter)."""
    return {name: path for name, path in programs.sources().items() if path.is_relative_to(R / "bench")}


def main() -> None:
    setup()
    parser = argparse.ArgumentParser()
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--extra", nargs="*", default=[])
    args = parser.parse_args()
    sources = bench_sources()
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
