"""The spill model's forecast beside the allocator's spill code, per function of every bench program.

`python3 tools/pressure.py LABEL=FLAG,FLAG ...` after `cargo build --release`: each argument is a run, its flags
`llrm-c`'s (none is the default target). A price is clocks per entry.
"""
import glob
import math
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LLRM = Path(subprocess.run([sys.executable, str(ROOT / "tools" / "llrmbin.py"), "bin"], capture_output=True, text=True, check=True).stdout.strip()) / "llrm-c"
ROW = re.compile(r"\[pressure\] (\S+) forecast peak (\d+) spilled (\d+) price (\d+); allocator (\d+) reloads, (\d+) stores, (\d+) remats, price (\d+)")


def runs(arguments):
    for argument in arguments:
        label, _, flags = argument.partition("=")
        yield label, [flag for flag in flags.split(",") if flag]


def rows(label, flags):
    for source in sorted(glob.glob(str(ROOT / "bench" / "*" / "*.c"))):
        program = os.path.basename(source)[:-2]
        done = subprocess.run([LLRM, *flags, "-O2", "-fno-inline-functions", "-S", "-o", os.devnull, source], capture_output=True, text=True, env={**os.environ, "LLRM_DEBUG": "pressure"})
        for line in done.stderr.splitlines():
            match = ROW.match(line)
            if match:
                yield (label, program, match.group(1), *map(int, match.groups()[1:]))


def main(arguments):
    for label, flags in runs(arguments):
        found = list(rows(label, flags))
        neither = sum(1 for r in found if r[5] == 0 and r[9] == 0)
        forecast_only = [r for r in found if r[5] > 0 and r[9] == 0]
        allocator_only = [r for r in found if r[5] == 0 and r[9] > 0]
        both = [r for r in found if r[5] > 0 and r[9] > 0]
        print(label, "functions", len(found), "both none", neither, "forecast only", len(forecast_only), "allocator only", len(allocator_only), "both", len(both))
        if both:
            print("  geomean forecast/actual price where both spill: %.2f" % math.exp(sum(math.log(r[5] / r[9]) for r in both) / len(both)))
        print("  largest allocator-only:", sorted(allocator_only, key=lambda r: -r[9])[:5])
        print("  largest forecast-only:", sorted(forecast_only, key=lambda r: -r[5])[:5])


if __name__ == "__main__":
    main(sys.argv[1:] or ["default="])
