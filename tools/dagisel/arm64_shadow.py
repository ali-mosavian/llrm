#!/usr/bin/env python3
"""Per function, the opcodes the arm64 DAG path selects against the pin's llc.

    tools/dagisel/arm64_shadow.py FILE.ll... [--llc PATH]

Runs `cargo run -p llrm-arm64 --example select_shadow` on each file (the module after the mid-end,
then the DAG path), feeds the printed module to the pin's `llc -O2 -stop-after=finalize-isel`, and
compares the machine opcodes per block, ignoring copies and bookkeeping; the same opcodes in another order
(the scheduler's) are `equal (order)`. Prints one line per
function: `equal`, `differ` with both lists, or `refused` with the reason.
"""
import argparse
import os
import re
import subprocess
import sys
import tempfile

BOOKKEEPING = {"COPY", "EXTRACT_SUBREG", "INSERT_SUBREG", "SUBREG_TO_REG", "IMPLICIT_DEF", "PHI", "KILL",
               "COPY_TO_REGCLASS", "REG_SEQUENCE", "ADJCALLSTACKDOWN", "ADJCALLSTACKUP", "LIFETIME_START", "LIFETIME_END"}
DEFAULT_LLC = os.path.expanduser("~/scratch/llvm-build-aarch64/bin/llc")


def ours(path):
    out = subprocess.run(["cargo", "run", "-q", "-p", "llrm-arm64", "--example", "select_shadow", "--", path],
                         capture_output=True, text=True)
    module = re.search(r"^MODULE\n(.*?)^ENDMODULE", out.stdout, re.S | re.M)
    if not module:
        sys.exit(f"{path}: the example failed:\n{out.stderr[-2000:]}")
    refused, blocks = {}, {}
    for line in out.stdout.splitlines():
        if m := re.match(r"@(\S+) REFUSED: (.*)", line):
            refused[m[1]] = m[2]
        elif m := re.match(r"@(\S+) bb(\d+): ?(.*)", line):
            blocks.setdefault(m[1], []).append([o for o in m[3].split() if o not in BOOKKEEPING])
    # The pin's llc runs its own CodeGenPrepare: it gets the module before our cgpaddr, not after.
    pre = subprocess.run(["cargo", "run", "-q", "-p", "llrm-arm64", "--example", "select_shadow", "--", path],
                         capture_output=True, text=True, env={**os.environ, "LLRM_NO_CGPADDR": "1"})
    pre_module = re.search(r"^MODULE\n(.*?)^ENDMODULE", pre.stdout, re.S | re.M)
    return (pre_module or module)[1], blocks, refused


def parse_pin(text):
    funcs, name = {}, None
    for line in text.splitlines():
        if m := re.match(r"name:\s+(\S+)", line):
            name = m[1]
            funcs[name] = []
        elif re.match(r"\s+bb\.\d+", line) and name:
            funcs[name].append([])
        elif name and funcs[name] and (m := re.match(r"\s+(?:(?:(?:dead|early-clobber) )?[%$]\S+(?::\S+)?(?:, (?:(?:dead|early-clobber) )?[%$]\S+)* = )?(?:\w+ )*([A-Z][A-Za-z0-9_]*)\b", line)):
            op = m[1]
            if op not in BOOKKEEPING and not op.startswith(("liveins", "successors")):
                funcs[name][-1].append(op)
    return funcs


def pin(llc, module):
    with tempfile.NamedTemporaryFile("w", suffix=".ll", delete=False) as f:
        f.write(module)
    out = subprocess.run([llc, "-O2", "-mtriple=arm64-apple-macosx", "-mcpu=apple-m1", "-stop-after=finalize-isel", f.name, "-o", "-"],
                         capture_output=True, text=True)
    os.unlink(f.name)
    if out.returncode:
        sys.exit(f"llc failed:\n{out.stderr[-2000:]}")
    return parse_pin(out.stdout)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("files", nargs="+")
    ap.add_argument("--llc", default=DEFAULT_LLC)
    args = ap.parse_args()
    for path in args.files:
        module, blocks, refused = ours(path)
        theirs = pin(args.llc, module)
        for name in sorted(set(blocks) | set(refused)):
            if name in refused:
                print(f"{path} @{name}: refused: {refused[name]}")
            elif blocks[name] == theirs.get(name):
                print(f"{path} @{name}: equal")
            elif [sorted(b) for b in blocks[name]] == [sorted(b) for b in theirs.get(name) or []]:
                print(f"{path} @{name}: equal (order)")
            else:
                print(f"{path} @{name}: differ\n  ours: {blocks[name]}\n  pin:  {theirs.get(name)}")


if __name__ == "__main__":
    main()
