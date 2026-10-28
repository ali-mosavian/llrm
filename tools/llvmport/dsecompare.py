#!/usr/bin/env python3
"""Compares DSE's removals, LLVM's `opt -passes=dse` against llrm's `llrm-mir --dse`, on generated functions over near pointers.

    tools/llvmport/dsecompare.py [--cases N] [--seed S] [--opt PATH] [--show K]

Each case is a function of a few blocks (diamonds and chains) whose instructions store constants to offsets of a local and of a global,
memset ranges, load cells and call an unknown function. Both sides print the stores and memsets left, each by the base and offset its
pointer names, its size and its value; the lists must be equal. Exit 1 on a difference.
"""
import os
import random
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DL = 'target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32"\n\n'


def operations(rng, block, count):
    """The lines of one block and its pointer names."""
    lines = []
    for n in range(count):
        base = rng.choice(["%a", "@g"])
        kind = rng.choice(["store"] * 6 + ["memset", "load", "call"])
        offset = rng.choice([0, 0, 1, 2, 4, 8])
        name = f"%p{block}_{n}"
        if kind == "store":
            bits = rng.choice([8, 16, 16, 32])
            value = rng.choice([0, 1, 2, 3, 7])
            lines.append(f"  {name} = getelementptr i8, ptr {base}, i16 {offset}\n  store i{bits} {value}, ptr {name}\n")
        elif kind == "memset":
            length = rng.choice([2, 4, 8])
            lines.append(f"  {name} = getelementptr i8, ptr {base}, i16 {offset}\n  call void @llvm.memset.p0.i16(ptr {name}, i8 {rng.choice([0, 1])}, i16 {length}, i1 false)\n")
        elif kind == "load":
            lines.append(f"  {name} = getelementptr i8, ptr {base}, i16 {offset}\n  %l{block}_{n} = load i8, ptr {name}\n  store volatile i8 %l{block}_{n}, ptr @s\n")
        else:
            lines.append("  call void @ext()\n")
    return "".join(lines)


def case(rng):
    shape = rng.choice(["chain", "diamond", "diamond", "triangle"])
    body = {"chain": 2, "diamond": 4, "triangle": 3}[shape]
    text = DL + "@g = global [64 x i8] zeroinitializer\n@s = global i8 0\ndeclare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\ndeclare void @ext()\n\ndefine i16 @f(i16 %x) {\nb0:\n  %a = alloca [64 x i8]\n"
    blocks = []
    for n in range(body):
        blocks.append(operations(rng, n, rng.randint(1, 5)))
    tail = "  %r = load i16, ptr @g\n  ret i16 %r\n" if rng.random() < 0.7 else "  ret i16 %x\n"
    if shape == "chain":
        text += blocks[0] + "  br label %b1\n\nb1:\n" + blocks[1] + tail
    elif shape == "triangle":
        text += blocks[0] + "  %c = icmp eq i16 %x, 0\n  br i1 %c, label %b1, label %b2\n\nb1:\n" + blocks[1] + "  br label %b2\n\nb2:\n" + blocks[2] + tail
    else:
        text += blocks[0] + "  %c = icmp eq i16 %x, 0\n  br i1 %c, label %b1, label %b2\n\nb1:\n" + blocks[1] + "  br label %b3\n\nb2:\n" + blocks[2] + "  br label %b3\n\nb3:\n" + blocks[3] + tail
    return text + "}\n"


def normalise(out):
    """Stores and memsets left, by base, offset, size, value."""
    names = {}
    for m in re.finditer(r"%(\w+) = getelementptr (?:inbounds )?i8, ptr (%a|@g), i16 (-?\d+)", out):
        names[m.group(1)] = (m.group(2), int(m.group(3)))
    found = []
    for m in re.finditer(r"store (?!volatile)i(\d+) (-?\w+), ptr (?:align \d+ )?(?:%(\w+)|(@g)|(%a))", out):
        base = names.get(m.group(3), (m.group(4) or m.group(5) or "?", 0))
        found.append(("store", base, int(m.group(1)), m.group(2)))
    for m in re.finditer(r"call void @llvm\.memset\.p0\.i\d+\(ptr (?:align \d+ )?(?:%(\w+)|(@g)|(%a)), i8 (\w+), i\d+ (\d+), i1 false\)", out):
        base = names.get(m.group(1), (m.group(2) or m.group(3), 0))
        found.append(("memset", base, int(m.group(5)), m.group(4)))
    return sorted(found, key=repr)


def main():
    arg = lambda flag, default: sys.argv[sys.argv.index(flag) + 1] if flag in sys.argv else default
    cases, seed, show = int(arg("--cases", 200)), int(arg("--seed", 1)), int(arg("--show", 4))
    opt = arg("--opt", str(Path.home() / "scratch/llvm-build/bin/opt"))
    mir = os.environ.get("LLRM_MIR", str(ROOT / "target/debug/llrm-mir"))
    rng, bad, shown = random.Random(seed), 0, 0
    for n in range(cases):
        text = case(rng)
        with tempfile.NamedTemporaryFile("w", suffix=".ll", delete=False) as handle:
            handle.write(text)
        theirs = subprocess.run([opt, "-passes=dse", "-S", handle.name], capture_output=True, text=True)
        ours = subprocess.run([mir, "--dse", handle.name], capture_output=True, text=True)
        os.unlink(handle.name)
        if theirs.returncode or ours.returncode:
            sys.exit(f"case {n}: opt rc {theirs.returncode} {theirs.stderr[-200:]} / llrm-mir rc {ours.returncode} {ours.stderr[-300:]}\n{text}")
        a, b = normalise(ours.stdout), normalise(theirs.stdout)
        if a != b:
            bad += 1
            if shown < show:
                shown += 1
                print(f"--- case {n}\n{text}\nllrm: {a}\nopt:  {b}")
    print(f"{cases} cases, {bad} differ")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
