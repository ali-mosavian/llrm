#!/usr/bin/env python3
"""tools/lsr_shadow.py PIN_OPT OURS_CMD FILE.ll...: runs the pin's `opt -passes=loop-reduce` and ours on each loop and
compares the shape (phis by type, instruction counts by opcode); prints both on a difference. OURS_CMD reads the file
named last, e.g. `target/release/examples/lsr`. Needs the same triple as OURS_CMD's target: --triple."""
import argparse
import collections
import re
import subprocess
import sys


def shape(text):
    counts = collections.Counter()
    for line in text.splitlines():
        line = line.strip()
        if not line or line[0] in ";}" or line.startswith(("define", "declare", "target", "source_filename", "attributes")):
            continue
        if re.match(r"^[\w.$-]+:", line):
            continue
        body = line.split("=", 1)[1].strip() if "=" in line.split("(")[0] else line
        op = body.split()[0]
        if op == "phi":
            op = "phi " + body.split()[1]
        counts[op] += 1
    return counts


parser = argparse.ArgumentParser()
parser.add_argument("pin_opt")
parser.add_argument("ours")
parser.add_argument("files", nargs="+")
parser.add_argument("--triple", required=True)
args = parser.parse_args()
bad = 0
for path in args.files:
    pin = subprocess.run([args.pin_opt, "-S", "-passes=loop-reduce", f"-mtriple={args.triple}", path], capture_output=True, text=True)
    ours = subprocess.run([args.ours, path], capture_output=True, text=True)
    if pin.returncode or ours.returncode:
        print(f"{path}: a tool failed\n{pin.stderr[:300]}{ours.stderr[:300]}")
        bad += 1
        continue
    same = shape(pin.stdout) == shape(ours.stdout)
    print(f"{path}: {'same' if same else 'DIFFERS'}")
    if not same:
        bad += 1
        print("--- pin\n" + pin.stdout + "--- ours\n" + ours.stdout)
sys.exit(1 if bad else 0)
