#!/usr/bin/env python3
"""Compares MemCpyOpt's memsets, LLVM's `opt -passes=memcpyopt` against llrm's `llrm-mir --memcpyopt`, on generated blocks of stores.

    tools/llvmport/mccompare.py [--cases N] [--seed S] [--opt PATH] [--show K]

Each case is one block of stores of a constant to offsets of one global, memsets, loads, and an unknown call, in a random order. Both sides
print the memsets made (offset, length, byte) and the stores left (offset, width, value); the lists must be equal. Exit 1 on a difference.
"""
import os
import random
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PIN = "d1106deb71cc81016406a1917d0508d2df296266"
DL = 'target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32"\n\n'


def case(rng):
    body, at = [], 0
    ops = rng.randint(3, 14)
    offset = 0
    for n in range(ops):
        kind = rng.choice(["store"] * 8 + ["memset", "load", "call"])
        if kind == "store":
            bits = rng.choice([8, 8, 8, 16, 32])
            value = rng.choice([0, 0, 0, -1, 1] + ([257] if bits == 16 else [0x01010101] if bits == 32 else []))
            if rng.random() < 0.15:
                offset += rng.choice([1, 2, 8])
            body.append((f"p{n}", offset, f"  %p{n} = getelementptr i8, ptr @a, i16 {offset}\n  store i{bits} {value}, ptr %p{n}\n"))
            offset += bits // 8
        elif kind == "memset":
            length = rng.choice([2, 4, 8])
            byte = rng.choice([0, 0, 1])
            body.append((f"p{n}", offset, f"  %p{n} = getelementptr i8, ptr @a, i16 {offset}\n  call void @llvm.memset.p0.i16(ptr %p{n}, i8 {byte}, i16 {length}, i1 false)\n"))
            offset += length
        elif kind == "load":
            body.append((None, 0, f"  %l{n} = load i8, ptr @a\n  store volatile i8 %l{n}, ptr @s\n"))
        else:
            body.append((None, 0, "  call void @ext()\n"))
    text = DL + "@a = global [256 x i8] zeroinitializer\n@s = global i8 0\ndeclare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\ndeclare void @ext()\n\ndefine i16 @f(i16 %x) {\nb0:\n"
    text += "".join(part for _, _, part in body)
    text += "  %r = load i16, ptr @a\n  ret i16 %r\n}\n"
    return text


def normalise(out):
    """Memsets and stores left, by the offset each pointer names."""
    names = {}
    for m in re.finditer(r"%(\w+) = getelementptr (?:inbounds )?i8, ptr @a, i16 (-?\d+)", out):
        names[m.group(1)] = int(m.group(2))
    names[None] = 0
    memsets, stores = [], []
    for m in re.finditer(r"call void @llvm\.memset\.p0\.i\d+\(ptr (?:align \d+ )?(?:%(\w+)|@a), i8 (\w+), i\d+ (\d+), i1 false\)", out):
        memsets.append((names.get(m.group(1), 0), int(m.group(3)), m.group(2)))
    for m in re.finditer(r"store (?!volatile)i(\d+) (-?\w+), ptr (?:align \d+ )?(?:%(\w+)|@a)", out):
        stores.append((names.get(m.group(3), 0), int(m.group(1)), m.group(2)))
    return sorted(memsets), sorted(stores)


def main():
    arg = lambda flag, default: sys.argv[sys.argv.index(flag) + 1] if flag in sys.argv else default
    cases, seed, show = int(arg("--cases", 200)), int(arg("--seed", 1)), int(arg("--show", 5))
    opt = arg("--opt", str(Path.home() / "scratch/llvm-build/bin/opt"))
    mir = os.environ.get("LLRM_MIR", str(ROOT / "target/debug/llrm-mir"))
    version = subprocess.run([opt, "--version"], capture_output=True, text=True).stdout
    if "24.0.0git" not in version:
        sys.exit(f"mccompare: opt is not the pinned build: {version.strip()}")
    rng, bad, shown = random.Random(seed), 0, 0
    for n in range(cases):
        text = case(rng)
        with tempfile.NamedTemporaryFile("w", suffix=".ll", delete=False) as handle:
            handle.write(text)
        theirs = subprocess.run([opt, "-passes=memcpyopt", "-S", handle.name], capture_output=True, text=True)
        ours = subprocess.run([mir, "--memcpyopt", handle.name], capture_output=True, text=True)
        os.unlink(handle.name)
        if theirs.returncode or ours.returncode:
            sys.exit(f"case {n}: opt rc {theirs.returncode} {theirs.stderr[-200:]} / llrm-mir rc {ours.returncode} {ours.stderr[-200:]}\n{text}")
        a, b = normalise(ours.stdout), normalise(theirs.stdout)
        if a != b:
            bad += 1
            if shown < show:
                shown += 1
                print(f"--- case {n}\n{text}\nllrm: {a}\nopt:  {b}")
    print(f"{cases} cases, {bad} differ")
    sys.exit(1 if bad else 0)


main()
