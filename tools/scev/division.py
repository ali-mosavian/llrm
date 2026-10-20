#!/usr/bin/env python3
"""SCEVDivision against the pinned `opt -passes='print<scev-division>'`.

    tools/scev/division.py [--bin PATH] [FILE.ll ...]    (default: tools/scev/division.ll)
    tools/scev/division.py --golden > tools/scev/division.txt    (what the Rust test reads)

Compares, per function and per signed division in order, the numerator, the
denominator, the quotient and the remainder as printed. Prints
`files total / compared / failed / missing`, then each mismatch. Exit 1 on any
mismatch or missing side.
"""
import os
import re
import subprocess
import sys
from pathlib import Path

PINNED = Path.home() / "scratch/llvm-build/bin/opt"
ROOT = Path(__file__).resolve().parent.parent.parent
LLRM = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "debug/llrm-mir"
FIELD = re.compile(r"^\s+(Numerator|Denominator|Quotient|Remainder): (.*)$")


def run(cmd):
    return subprocess.run(cmd, capture_output=True, text=True)


def parse(text):
    """{function: [(field, text), ...]} in print order."""
    out, function = {}, None
    for line in text.splitlines():
        if line.startswith("Printing analysis 'Scalar Evolution Division' for function '"):
            function = line.split("'")[3].lstrip("@")
            out[function] = []
        elif function is not None and (m := FIELD.match(line)):
            out[function].append((m[1], m[2]))
    return out


def golden(path, binary):
    """The pinned opt's fields, function by function: what division.txt keeps for the Rust test."""
    out = ""
    for function, fields in parse(run([binary, "-passes=print<scev-division>", "-disable-output", path]).stderr).items():
        out += f"@{function}\n" + "".join(f"  {name}: {text}\n" for name, text in fields)
    return out


def main():
    args, binary = sys.argv[1:], str(PINNED)
    if args[:1] == ["--golden"]:
        sys.stdout.write(golden(str(Path(__file__).with_name("division.ll")), binary))
        return 0
    if args[:1] == ["--bin"]:
        binary, args = args[1], args[2:]
    files = args or [str(Path(__file__).with_name("division.ll"))]
    failed = missing = compared = 0
    for path in files:
        want = run([binary, "-passes=print<scev-division>", "-disable-output", path])
        got = run([str(LLRM), "--scev-division", path])
        if want.returncode or got.returncode:
            missing += 1
            print(f"MISSING {path}: opt {want.returncode} {want.stderr[:200]} llrm {got.returncode} {got.stderr[:200]}")
            continue
        compared += 1
        a, b = parse(want.stderr), parse(got.stdout)
        if a != b:
            failed += 1
            for function in sorted(set(a) | set(b)):
                for n, (x, y) in enumerate(zip(a.get(function, []), b.get(function, []))):
                    if x != y:
                        print(f"{path} @{function} #{n // 4}: opt {x} llrm {y}")
                if len(a.get(function, [])) != len(b.get(function, [])):
                    print(f"{path} @{function}: {len(a.get(function, []))} fields in opt, {len(b.get(function, []))} in llrm")
    print(f"{len(files)} files / {compared} compared / {failed} failed / {missing} missing")
    return 1 if failed or missing else 0


if __name__ == "__main__":
    sys.exit(main())
