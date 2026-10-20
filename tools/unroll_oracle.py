#!/usr/bin/env python3
"""Complete unrolling against the pinned `opt -passes=loop-unroll` on the same loops.

    tools/unroll_oracle.py [FILE.ll ...]    (default: tests/fixtures/unroll)

Each file starts with `; unroll HEADER count N exits BLOCK=T ...`: the loop to
unroll, its copies, and how often each named exiting block runs when it leaves.
N must equal opt's constant max backedge-taken count of that loop + 1, and opt
must have unrolled that loop (the premise: otherwise the input says nothing).
Both results go through `instcombine,simplifycfg,adce`; their blocks and
instructions per opcode must be equal. A control runs rotated.ll with one
copy too many and must differ: an oracle that cannot fail says nothing. The
corpus must hold each shape the translation claims (REQUIRED). Prints
`files total / compared / failed / missing` first, then each mismatch. Exit 1
on any failure or a missing side.
"""
import collections
import targetdesc
import glob
import re
import subprocess
import sys
from pathlib import Path

REQUIRED = {
    "header_tested", "rotated", "unknown_trips", "two_exits", "nested_inner", "switch_exit",
    "lcssa_used_outside", "three_exits",
}
PINNED = Path.home() / "scratch/llvm-build/bin/opt"
TARGET = targetdesc.triple_cpu()  # llrm's default target (crates/target/*/datalayout.toml): a dump has no `target triple`, and opt costs for x86-64 without one
COMMIT = "24.0.0git"
ROOT = Path(__file__).resolve().parent.parent
LLRM = ROOT / "target/debug/llrm-mir"


def run(cmd, stdin=None):
    return subprocess.run(cmd, input=stdin, capture_output=True, text=True)


def shape(ir):
    blocks = 0
    ops = collections.Counter()
    for line in ir.splitlines():
        s = line.strip()
        if re.match(r"^[\w.$-]+:", s):
            blocks += 1
        elif s and not s.startswith((";", "define", "}", "declare", "source_filename", "target")):
            ops[re.sub(r"^%[\w.$-]+ = ", "", s).split()[0]] += 1
    return blocks, ops


def cleaned(ir):
    r = run([str(PINNED), *TARGET, "-passes=instcombine,simplifycfg,adce", "-S"], stdin=ir)
    return r.stdout if r.returncode == 0 else None


def loops_of(ir):
    return run([str(PINNED), *TARGET, "-passes=print<loops>", "-disable-output"], stdin=ir).stderr.count("Loop at depth")


def llrm_unrolled(path, header, count, exits):
    cmd = [str(LLRM), "--unroll", f"{header}:{count}"]
    for block, trips in exits:
        cmd += ["--exit", f"{block}={trips}"]
    return run(cmd + [path])


def main():
    version = run([str(PINNED), "--version"]).stdout
    if COMMIT not in version:
        raise SystemExit(f"unroll_oracle: {PINNED} is not LLVM {COMMIT}")
    print([l.strip() for l in version.splitlines() if "version" in l][0], f"(pinned {PINNED})")
    files = sys.argv[1:] or sorted(glob.glob(str(ROOT / "tests/fixtures/unroll/*.ll")))
    compared = failed = missing = 0
    bad = []
    if not sys.argv[1:]:
        absent = REQUIRED - {Path(f).stem for f in files}
        if absent:
            failed += len(absent)
            bad.append(f"the corpus misses shapes: {sorted(absent)}")
    first = None
    for f in files:
        text = Path(f).read_text()
        spec = re.match(r"; unroll (\S+) count (\d+) exits(.*)", text)
        if not spec:
            missing += 1
            bad.append(f"{f}: no `; unroll HEADER count N exits ...` line")
            continue
        header, count = spec.group(1), int(spec.group(2))
        exits = [(b, int(n)) for b, n in re.findall(r"(\S+)=(\d+)", spec.group(3))]
        scev = run([str(PINNED), *TARGET, "-passes=print<scalar-evolution>", "-disable-output", f])
        maxes = dict(re.findall(r"Loop %(\S+): constant max backedge-taken count is i\d+ (\d+)", scev.stderr))
        want = run([str(PINNED), *TARGET, "-passes=loop-unroll", "-S", f])
        if scev.returncode or want.returncode or header not in maxes:
            missing += 1
            bad.append(f"{f}: opt rc {scev.returncode}/{want.returncode}, no constant max count for %{header}")
            continue
        if int(maxes[header]) + 1 != count:
            failed += 1
            bad.append(f"{f}: premise: the file says {count} copies, opt's max count + 1 is {int(maxes[header]) + 1}")
            continue
        if loops_of(want.stdout) != loops_of(text) - 1:
            failed += 1
            bad.append(f"{f}: premise: opt did not unroll %{header} completely")
            continue
        got = llrm_unrolled(f, header, count, exits)
        if got.returncode:
            missing += 1
            bad.append(f"{f}: llrm-mir rc {got.returncode}: {got.stderr[:200]}")
            continue
        a, b = cleaned(want.stdout), cleaned(got.stdout)
        if a is None or b is None:
            missing += 1
            bad.append(f"{f}: opt cannot read {'its own' if a is None else 'llrm-mir'}'s output")
            continue
        compared += 1
        if Path(f).stem == "rotated":
            first = (f, header, count, exits, a)
        if shape(a) != shape(b):
            failed += 1
            bad.append(f"{f}: count {count}\n  opt  : {shape(a)}\n  llrm : {shape(b)}")
    if first:
        f, header, count, exits, a = first
        control = llrm_unrolled(f, header, count + 1, [(b, n + 1) for b, n in exits])
        if control.returncode == 0 and cleaned(control.stdout) is not None and shape(cleaned(control.stdout)) == shape(a):
            failed += 1
            bad.append(f"control: {f} with {count + 1} copies equals opt's {count}: the oracle cannot fail")
    print(f"files {len(files)} total / {compared} compared / {failed} failed / {missing} missing")
    print("\n".join(bad[:20]))
    sys.exit(1 if failed or missing else 0)


main()
