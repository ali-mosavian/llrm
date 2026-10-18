#!/usr/bin/env python3
"""The SCEV of each value against the pinned `opt -passes='print<scalar-evolution>'`.

    tools/scev/values.py [--bin PATH] [FILE.ll ...]    (default: the analysis corpus)

Compares, per non-compare instruction, the whole "Classifying expressions"
line: the expression, its ranges, its value at the loop's scope, its exit
value and its loop dispositions. Prints
`files total / compared / failed / missing`, then each mismatch. Exit 1 on any
mismatch or missing side; the pinned opt is refused if it is not LLVM 24.0.0git.
"""
import glob
import re
import subprocess
import sys
from pathlib import Path

PINNED = Path.home() / "scratch/llvm-build/bin/opt"
COMMIT = "24.0.0git"
ROOT = Path(__file__).resolve().parent.parent.parent


def run(cmd):
    return subprocess.run(cmd, capture_output=True, text=True)


def parse_opt(text):
    """{function: [(name, 'expr U: u S: s')]} from the classifying section."""
    out, function, name = {}, None, None
    for line in text.splitlines():
        if line.startswith("Classifying expressions for: @"):
            function = line.split("@", 1)[1]
            out[function] = []
        elif line.startswith("Determining loop execution counts"):
            function = None
        elif function is not None and line.startswith("  %") and " = " in line:
            name = line.strip().split(" = ", 1)[0]
        elif function is not None and line.startswith("  -->  ") and name:
            body = line[len("  -->  "):].replace("\t\t", "\t")
            out[function].append((name, body))
            name = None
    return out


def parse_llrm(text):
    out, function = {}, None
    for line in text.splitlines():
        if line.startswith("@"):
            function = line[1:]
            out[function] = []
        else:
            name, body = line.split("\t", 1)
            out[function].append((name, body.replace("\t\t", "\t")))
    return out


def main():
    args = sys.argv[1:]
    binary = ROOT / "target/release/llrm-mir"
    if args[:1] == ["--bin"]:
        binary, args = Path(args[1]), args[2:]
    version = run([str(PINNED), "--version"]).stdout
    if COMMIT not in version:
        raise SystemExit(f"values.py: {PINNED} is not LLVM {COMMIT}")
    print([l.strip() for l in version.splitlines() if "version" in l][0])
    files = args or sorted(glob.glob(str(ROOT / "crates/opt/llrm-analysis/corpus/*/*.ll")))
    compared = failed = missing = values = addrecs = 0
    bad = []
    for f in files:
        want = run([str(PINNED), "-passes=print<scalar-evolution>", "-disable-output", f])
        got = run([str(binary), "--scev-values", f])
        if want.returncode or got.returncode:
            missing += 1
            why = want.stderr if want.returncode else got.stderr
            bad.append(f"{f}: opt rc {want.returncode}, llrm-mir rc {got.returncode}: {why[-200:]}")
            continue
        compared += 1
        a, b = parse_opt(want.stderr), parse_llrm(got.stdout)
        wrong = None
        for function in a:
            for (n1, e1), (n2, e2) in zip(a[function], b.get(function, [])):
                values += 1
                addrecs += "<%" in e1 and ",+," in e1
                if (n1, e1) != (n2, e2) and wrong is None:
                    wrong = f"{f} @{function}\n  opt  : {n1} {e1[:300]}\n  llrm : {n2} {e2[:300]}"
            if len(a[function]) != len(b.get(function, [])) and wrong is None:
                wrong = f"{f} @{function}: {len(a[function])} values in opt, {len(b.get(function, []))} in llrm"
        if wrong:
            failed += 1
            bad.append(wrong)
    print(f"files {len(files)} total / {compared} compared / {failed} failed / {missing} missing")
    print(f"values compared {values}, add recurrences in opt's {addrecs}")
    if not addrecs:
        # An oracle that sees no recurrence says nothing about createAddRecFromPHI.
        bad.append("premise: opt's output holds no add recurrence")
        failed += 1
    print("\n".join(bad[:20]))
    sys.exit(1 if failed or missing else 0)


main()
