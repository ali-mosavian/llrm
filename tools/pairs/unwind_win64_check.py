#!/usr/bin/env python3
"""Win64 unwind data against our own prologue, no pin: each push, stack allocation, frame-register set-up and register save the prologue makes has an unwind code at the offset after it, and each code has such an instruction.

    unwind_win64_check.py OBJ... [--tools DIR]     llvm-objdump in DIR (default ~/scratch/llvm-build-aarch64/bin)
"""
import argparse
import re
import subprocess
import sys
from pathlib import Path

from unwind_win64_diff import code_of, symbols_of

REGS = {"rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", *(f"r{n}" for n in range(8, 16))}


def codes_of(part: str) -> list[tuple[int, str, str]]:
    """(offset, kind, operands) of each `0xOFF: UOP_Kind operands` line of one function table."""
    return [(int(m.group(1), 16), m.group(2), m.group(3).strip()) for m in re.finditer(r"0x([0-9a-f]+): UOP_(\w+)([^\n]*)", part)]


def expected(function: list[str], size: int) -> list[tuple[int, str, str]]:
    """(offset after the instruction, kind, operands) the unwind data must hold for the instructions of `function` ("off text" lines) that start inside its prolog of `size` bytes."""
    out = []
    for i, line in enumerate(function):
        off, text = line.split(" ", 1)
        if int(off, 16) >= size:
            break
        # the last instruction of a block ends where the next label starts: the prolog's size, as the label follows it
        end = int(function[i + 1].split()[0], 16) if i + 1 < len(function) else size
        # `push rax` is the compiler's one-slot allocation, not a saved register; `sub rax` after the stack probe is the allocation
        if text == "pushq %rax":
            out.append((end, "Alloc", ""))
        elif (m := re.match(r"pushq? %(\w+)$", text)) and m.group(1) in REGS:
            out.append((end, "PushNonVol", m.group(1).upper()))
        elif re.match(r"subq (\$(0x[0-9a-f]+|\d+)|%rax), %rsp$", text):
            out.append((end, "Alloc", ""))
        elif re.match(r"(movq %rsp, %rbp|leaq (-?0x[0-9a-f]+)?\(%rsp\), %rbp)$", text):
            out.append((end, "SetFPReg", ""))
        elif m := re.match(r"movaps %(xmm\d+), (-?0x[0-9a-f]+|0)?\(%r(sp|bp)\)$", text):
            out.append((end, "SaveXMM128", m.group(1).upper()))
        elif m := re.match(r"movq %(\w+), (-?0x[0-9a-f]+|0)?\(%r(sp|bp)\)$", text):
            if m.group(1) in REGS:
                out.append((end, "SaveNonVol", m.group(1).upper()))
    return out


def check(code: dict[int, list[str]], symbols: dict[str, int], tables: str) -> list[str]:
    """The problems of one object: its `-d` code by start, `-t` symbols, `-u` text."""
    names = {offset: name for name, offset in symbols.items()}
    problems = []
    for part in re.split(r"\n\s*Start Address:", tables)[1:]:
        label = part.split()[0]
        start = symbols.get(label, 0 if label == ".text" else None)
        size = re.search(r"Size of prolog: (\d+)", part)
        if start is None or size is None or start not in code:
            continue
        name = names.get(start, label)
        listed = [off for off, _, _ in codes_of(part)]
        if listed != sorted(listed, reverse=True):
            problems.append(f"{name}: unwind codes are not listed last instruction first: offsets {listed}")
        have = sorted((off, kind, ops.split()[0].upper() if kind in ("PushNonVol", "SaveNonVol", "SaveXMM128") else ops) for off, kind, ops in codes_of(part))
        want = sorted(e for e in expected(code[start], int(size.group(1))) if e[0] is not None)
        # an allocation prints in slots of eight (AllocLarge) or bytes+8 (AllocSmall): compare by kind and offset
        norm = lambda rows: [(o, "Alloc" if k.startswith("Alloc") else k, "" if k.startswith("Alloc") else a) for o, k, a in rows]
        if norm(have) != norm(want):
            problems.append(f"{name}: unwind codes {norm(have)} but the prologue makes {norm(want)}")
    tabled = {symbols.get(part.split()[0], 0 if part.split()[0] == ".text" else None) for part in re.split(r"\n\s*Start Address:", tables)[1:]}
    for name, start in symbols.items():
        first = code.get(start, [""])[0].split(" ", 1)[-1]
        if start not in tabled and not re.match(r"L\d", name) and re.match(r"(pushq? %\w+|subq .*, %rsp)$", first):
            problems.append(f"{name}: the prologue starts with `{first}` and the object has no unwind data for it")
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("objects", nargs="+")
    parser.add_argument("--tools", default=str(Path.home() / "scratch/llvm-build-aarch64/bin"))
    args = parser.parse_args()
    bad = checked = 0
    for obj in args.objects:
        dump = lambda *flags: subprocess.run([f"{args.tools}/llvm-objdump", *flags, obj], capture_output=True, text=True, check=True).stdout
        tables = dump("-u")
        problems = check(code_of(dump("-d", "--no-show-raw-insn")), symbols_of(dump("-t")), tables)
        checked += tables.count("Start Address:")
        bad += len(problems)
        for problem in problems:
            print(f"{obj}: {problem}")
    print(f"unwind_win64_check: {checked} functions, {bad} inconsistent")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
