#!/usr/bin/env python3
"""Delinearization against the pinned `opt -passes='print<delinearization>'`.

    tools/scev/delinearize.py [--bin PATH] [FILE.ll ...]    (default: tools/scev/delinearize.ll)
    tools/scev/delinearize.py --golden > tools/scev/delinearize.txt    (what the Rust test reads)

Compares, per function and per access in order, the access function, the
array declaration, the subscripts and the validation as printed (the `Inst:`
line is the instruction's text, which the two sides number differently).
Prints `files total / compared / failed / missing`, then each mismatch.
Exit 1 on any mismatch or missing side.
"""
import os
import subprocess
import sys
from pathlib import Path

PINNED = Path.home() / "scratch/llvm-build/bin/opt"
ROOT = Path(__file__).resolve().parent.parent.parent
LLRM = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "debug/llrm-mir"
HEADER = "Printing analysis 'Delinearization' for function '"


def run(cmd):
    return subprocess.run(cmd, capture_output=True, text=True)


def parse(text):
    """{function: [line, ...]} without the Inst lines and the blank ones."""
    out, function = {}, None
    for line in text.splitlines():
        if line.startswith(HEADER):
            function = line[len(HEADER):].split("'")[0].lstrip("@")
            out[function] = []
        elif function is not None and line and not line.startswith("Inst:"):
            out[function].append(line)
    return out


def golden(path, binary):
    """The pinned opt's lines, function by function: what delinearize.txt keeps for the Rust test."""
    out = ""
    for function, lines in parse(run([binary, "-passes=print<delinearization>", "-disable-output", path]).stderr).items():
        out += f"@{function}\n" + "".join(f"  {line}\n" for line in lines)
    return out


def main():
    args, binary = sys.argv[1:], str(PINNED)
    if args[:1] == ["--bin"]:
        binary, args = args[1], args[2:]
    if args[:1] == ["--golden"]:
        sys.stdout.write(golden(str(Path(__file__).with_name("delinearize.ll")), binary))
        return 0
    files = args or [str(Path(__file__).with_name("delinearize.ll"))]
    failed = missing = compared = 0
    for path in files:
        want = run([binary, "-passes=print<delinearization>", "-disable-output", path])
        got = run([str(LLRM), "--delinearize", path])
        if want.returncode or got.returncode:
            missing += 1
            print(f"MISSING {path}: opt {want.returncode} {want.stderr[:200]} llrm {got.returncode} {got.stderr[:200]}")
            continue
        compared += 1
        a, b = parse(want.stderr), parse(got.stdout)
        if a != b:
            failed += 1
            for function in sorted(set(a) | set(b)):
                x, y = a.get(function, []), b.get(function, [])
                for n in range(max(len(x), len(y))):
                    one, two = (x[n] if n < len(x) else "<none>"), (y[n] if n < len(y) else "<none>")
                    if one != two:
                        print(f"{path} @{function} line {n}: opt {one!r} llrm {two!r}")
    print(f"{len(files)} files / {compared} compared / {failed} failed / {missing} missing")
    return 1 if failed or missing else 0


if __name__ == "__main__":
    sys.exit(main())
