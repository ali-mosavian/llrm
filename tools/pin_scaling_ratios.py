"""`python3 tools/pin_scaling_ratios.py [--opt PATH] [--clang PATH]` rewrites tools/pin_scaling_ratios.json.

The pin's own growth per pass on the loop-scaling source (tools/test_loop_scaling.py: SMALL -> LARGE sequential counted loops): `opt
-passes='default<O2>' -time-passes` on the IR the pin's clang emits, the median of RUNS runs. A pass of ours that grows more than the pin's
larger 4x ratio x 1.5 grows where LLVM's does not; the test reads the data file, so it needs no pin binary."""
import argparse
import json
import re
import statistics
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import test_loop_scaling as scaling  # noqa: E402

# Our step name (compile_map / `[instr]` rows) -> the pin's pass in -time-passes.
PASSES = {
    "mir loopfullunroll": "LoopFullUnrollPass",
    "mir rotate": "LoopRotatePass",
    "mir indvars": "IndVarSimplifyPass",
    "mir loopidioms": "LoopIdiomRecognizePass",
    "mir jumpthreading": "JumpThreadingPass",
    "mir constraint-elimination": "ConstraintEliminationPass",
}
RUNS = 9


def pass_seconds(opt: str, ir: Path) -> dict[str, float]:
    done = subprocess.run([opt, "-passes=default<O2>", "-time-passes", "-disable-output", str(ir)], capture_output=True, text=True, check=True)
    found = {}
    for line in done.stderr.splitlines():
        row = re.match(r"\s*([\d.]+) \(\s*[\d.]+%\)\s+([\d.]+) \(\s*[\d.]+%\)\s+([\d.]+) \(\s*[\d.]+%\)\s+([\d.]+) \(\s*[\d.]+%\)\s+(\w+Pass)\s*$", line)
        if row:
            found[row.group(5)] = float(row.group(3))  # user+sys: the pass's own CPU seconds
    return found


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--opt", required=True)
    ap.add_argument("--clang", required=True)
    args = ap.parse_args()
    sizes = {}
    with tempfile.TemporaryDirectory() as work:
        for k in (scaling.SMALL, scaling.LARGE, scaling.LARGE * 4):
            c, ll = Path(work) / f"k{k}.c", Path(work) / f"k{k}.ll"
            c.write_text(scaling.sequential(k))
            subprocess.run([args.clang, "-m32", "-w", "-O2", "-Xclang", "-disable-llvm-passes", "-S", "-emit-llvm", str(c), "-o", str(ll)], check=True)
            runs = [pass_seconds(args.opt, ll) for _ in range(RUNS)]
            sizes[k] = {name: statistics.median(r.get(name, 0.0) for r in runs) for name in PASSES.values()}
    sha = subprocess.run([args.clang, "--version"], capture_output=True, text=True, check=True).stdout
    small, large, huge = scaling.SMALL, scaling.LARGE, scaling.LARGE * 4
    # Two 4x steps: the timer reads 0.1 ms, so a small pass at SMALL is mostly rounding; the larger of the two ratios is the bar.
    ratio = {}
    for ours, theirs in PASSES.items():
        steps = [sizes[b][theirs] / sizes[a][theirs] for a, b in ((small, large), (large, huge)) if sizes[a][theirs] >= 0.0005]
        if steps:
            ratio[ours] = round(max(steps), 1)
    out = {"pin": re.search(r"([0-9a-f]{40})", sha).group(1), "small": small, "large": large, "huge": huge,
           "seconds": {str(k): v for k, v in sizes.items()}, "ratio": ratio}
    (HERE / "pin_scaling_ratios.json").write_text(json.dumps(out, indent=1) + "\n")
    print(json.dumps(out["ratio"]))


if __name__ == "__main__":
    main()
