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
    pre = subprocess.run(["cargo", "run", "-q", "-p", "llrm-arm64", "--example", "select_shadow", "--", path, "-fno-llrm-cgpaddr"],
                         capture_output=True, text=True)
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


# The cpu of the table (dagisel.toml `cpus`): the pin's default cpu has no LSE or RCPC, so atomics
# selected for apple-m1 were compared with load/store-exclusive loops.
PIN_FLAGS = ["-O2", "-mtriple=arm64-apple-macosx", "-mcpu=apple-m1", "-stop-after=finalize-isel"]


class OracleFailed(Exception):
    """The pin's llc could not read or compile the module."""


def pin(llc, module):
    with tempfile.NamedTemporaryFile("w", suffix=".ll", delete=False) as f:
        f.write(module)
    out = subprocess.run([llc, *PIN_FLAGS, f.name, "-o", "-"],
                         capture_output=True, text=True)
    os.unlink(f.name)
    if out.returncode:
        raise OracleFailed(out.stderr[-2000:])
    return parse_pin(out.stdout)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("files", nargs="+")
    ap.add_argument("--llc", default=os.path.join(os.environ.get("LLVM_BUILD", ""), "bin", "llc"), help="the pin's llc (default: $LLVM_BUILD/bin/llc)")
    args = ap.parse_args()
    if not os.path.exists(args.llc):
        sys.exit(f"no llc at {args.llc!r}: pass --llc or set LLVM_BUILD to the pin's build")
    for path in args.files:
        module, blocks, refused = ours(path)
        try:
            theirs = pin(args.llc, module)
        except OracleFailed as why:
            print(f"{path}: oracle failed: {str(why).splitlines()[0] if str(why) else '?'}")
            continue
        print("\n".join(compare(path, blocks, refused, theirs)))


def compare(path, blocks, refused, theirs):
    """One line per function: equal, equal (order), differ (both lists follow) or refused."""
    lines = []
    for name in sorted(set(blocks) | set(refused)):
        if name in refused:
            lines.append(f"{path} @{name}: refused: {refused[name]}")
        elif blocks[name] == theirs.get(name):
            lines.append(f"{path} @{name}: equal")
        elif [sorted(b) for b in blocks[name]] == [sorted(b) for b in theirs.get(name) or []]:
            lines.append(f"{path} @{name}: equal (order)")
        else:
            lines.append(f"{path} @{name}: differ\n  ours: {blocks[name]}\n  pin:  {theirs.get(name)}")
    return lines

if __name__ == "__main__":
    main()
