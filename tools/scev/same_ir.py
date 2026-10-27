#!/usr/bin/env python3
"""Same-IR cost of one pass, ours against the pin, in callgrind Ir.

    tools/scev/same_ir.py --ours llrm-pass --dumps DIR --slot indvars@738 \\
        --ours-toggle '*indvars*' --pin-passes indvars --pin-frame IndVarSimplifyPass::run

For each `--dump` file named SLOT, the file before it in its directory is the pass's input. Ours runs `llrm-pass --order SLOT`
with collection toggled on the pass's frames; the pin runs `opt -passes=PASSES` and the Ir of PIN-FRAME inclusive is read
back. MIR-only syntax is stripped for the pin. Prints Ir per body and the sums. Needs valgrind and callgrind_annotate.
"""
import argparse
import re
import subprocess
import sys
import tempfile
from pathlib import Path

DATALAYOUT = 'target datalayout = "e-p:32:32-i64:32-f80:32-n8:16:32-S32"\ntarget triple = "i486-unknown-linux-gnu"\n'


def run(cmd):
    return subprocess.run(cmd, capture_output=True, text=True)


def first_total(out, pattern=None, inclusive=False):
    annotate = ["callgrind_annotate"] + (["--inclusive=yes"] if inclusive else []) + [str(out)]
    text = run(annotate).stdout
    for line in text.splitlines():
        if (pattern is None and "PROGRAM TOTALS" in line) or (pattern and re.search(pattern, line)):
            return int(line.split()[0].replace(",", ""))
    return None


def pin_input(text):
    text = re.sub(r" nearcode|cc \d+ | addrspace\(1\)", "", text).replace("define internal", "define")
    return DATALAYOUT + text


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--ours", required=True)
    parser.add_argument("--pin", default=str(Path.home() / "scratch/llvm-build/bin/opt"))
    parser.add_argument("--dumps", required=True, help="a --dump directory; its walk/*/ files are read")
    parser.add_argument("--slot", required=True, help="e.g. indvars@738")
    parser.add_argument("--ours-toggle", required=True)
    parser.add_argument("--pin-passes", required=True)
    parser.add_argument("--pin-frame", required=True)
    parser.add_argument("--mode", default="32")
    args = parser.parse_args()
    rows = []
    for stage in sorted(Path(args.dumps).glob(f"walk/*/*-r01-{args.slot}.ll")):
        number = int(stage.name[:3])
        if number < 2:
            continue
        before = next(stage.parent.glob(f"{number - 1:03d}-*.ll"))
        with tempfile.TemporaryDirectory() as tmp:
            out_ours, out_pin = Path(tmp, "ours.out"), Path(tmp, "pin.out")
            run(["valgrind", "--tool=callgrind", f"--callgrind-out-file={out_ours}", f"--toggle-collect={args.ours_toggle}",
                 args.ours, "-O2", "--mode", args.mode, "--order", args.slot, str(before)])
            source = Path(tmp, "in.ll")
            source.write_text(pin_input(before.read_text()))
            run(["valgrind", "--tool=callgrind", f"--callgrind-out-file={out_pin}", args.pin,
                 f"-passes={args.pin_passes}", "-S", "-o", "/dev/null", str(source)])
            ours = first_total(out_ours)
            pin = first_total(out_pin, args.pin_frame, inclusive=True)
        if ours is not None and pin is not None:
            rows.append((f"{stage.parent.name}_{number:03d}", ours, pin))
            print(f"{rows[-1][0]}\tours {ours:>11,}\tpin {pin:>11,}\t{ours / pin:.2f}x")
    if rows:
        ours, pin = sum(r[1] for r in rows), sum(r[2] for r in rows)
        print(f"sum\tours {ours:>11,}\tpin {pin:>11,}\t{ours / pin:.2f}x ({len(rows)} bodies)")
    return 0 if rows else 1


if __name__ == "__main__":
    sys.exit(main())
