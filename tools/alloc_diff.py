#!/usr/bin/env python3
"""Where two listings of one function first differ in allocation: copies, constants, stack traffic.

    tools/alloc_diff.py OURS.txt PIN.txt [--context 2] [--max 12]

Each file is `objdump -d` text (either syntax) or `-S` assembly of ONE function, produced from the SAME IR. Registers,
immediates and branch targets are erased, the two instruction streams are aligned, and every hunk that is not equal is
classified: `copy` (register to register), `const` (`xor r,r`, `mov r,imm`, `movz`), `spill` (a store to the stack),
`reload` (a load from it), `other`. The summary gives the count of each class per side; the hunks name the first place the
streams part, with the instructions around it. It does not match virtual registers: a hunk is a difference of the final
code, not a decision of the allocator, and says so.
"""

from __future__ import annotations

import argparse
import difflib
import re
import sys
from collections import Counter
from pathlib import Path

REGISTER = re.compile(
    r"\b(?:r\d+[dwb]?|[re]?[abcd]x|[abcd][lh]|[re]?[sd]il?|[re]?[sb]pl?|[xyz]mm\d+|[xwqdshb]\d+(?:\.\d*[bhsd])?|xzr|wzr)\b"
)
STACK = re.compile(r"\[\s*(?:r|e)?sp\b")
NUMBER = re.compile(r"(?<![\w.])-?(?:0x[0-9a-f]+|\d+)\b")
BRANCH = re.compile(r"^((?:j[a-z]+|b(?:\.[a-z]+)?|bl|call|cb[n]?z|tb[n]?z)\b)(?: .*)?$")
LOAD = re.compile(r"^(?:ldr[bh]?|ldp|ldur[bh]?|ld[a-z0-9]*)\b")
STORE = re.compile(r"^(?:str[bh]?|stp|stur[bh]?|st[a-z0-9]*)\b")
MOVES = r"(?:mov|movs[sd]|movq|movd|movaps|movups|movdqa|movdqu)"
CLASSES = ("copy", "const", "spill", "reload", "other")


def instructions(text: str) -> list[str]:
    """The instruction text of each line of a listing; labels, headers and directives are dropped."""
    out = []
    for line in text.splitlines():
        if line.count("\t") >= 2:
            body = " ".join(part.strip() for part in line.split("\t")[2:])
        else:
            body = line.strip()
        body = re.split(r"\s(?:#|//|;)", body)[0].strip()
        if not body or body.endswith(":") or body.startswith(".") or "file format" in body or body.startswith("Disassembly"):
            continue
        out.append(re.sub(r"\s+", " ", body))
    return out


def shape(insn: str) -> str:
    """The instruction with registers, numbers and targets erased; the stack pointer survives as `[SP`."""
    text = re.sub(r"<[^>]*>", "", insn.lower())
    text = STACK.sub("[SP", text)
    text = REGISTER.sub("R", text)
    text = NUMBER.sub("N", text).replace("#", "")
    text = BRANCH.sub(r"\1 T", text)
    text = re.sub(r"\s*,\s*", ", ", text)
    return re.sub(r"\s+", " ", text).strip()


def kind(insn: str) -> str:
    """copy | const | spill | reload | other, from the shape of one instruction."""
    text = shape(insn)
    if "[SP" in text:
        if LOAD.match(text):
            return "reload"
        if STORE.match(text):
            return "spill"
        first = text.split(" ", 1)[1].split(",", 1)[0] if " " in text else ""
        if "[SP" in first:  # x86: the stack operand is the destination of a move, or a read-modify-write
            return "spill" if re.match(rf"^{MOVES}\b", text) else "other"
        return "reload"  # a load, or an operation with the stack operand folded in (`add eax, [rsp-4]`)
    if re.match(r"^(?:xor|pxor|xorps|eor) R, R(?:, R)?$", text) or re.match(r"^(?:mov|movz|movk|movabs|fmov|mvn) R, N$", text):
        return "const"
    if re.match(r"^(?:mov|movaps|movapd|movdqa|fmov|orr|mov\.16b) R, R(?:, R)?$", text):
        return "copy"
    return "other"


def counts(insns: list[str]) -> Counter:
    return Counter(kind(one) for one in insns)


def hunks(ours: list[str], pin: list[str]):
    """The non-equal opcodes of the aligned shapes: (tag, ours start, ours end, pin start, pin end)."""
    matcher = difflib.SequenceMatcher(None, [shape(one) for one in ours], [shape(one) for one in pin], autojunk=False)
    return [op for op in matcher.get_opcodes() if op[0] != "equal"]


def report(ours: list[str], pin: list[str], context: int = 2, limit: int = 12) -> str:
    """The summary and the first `limit` hunks that change the count of some class, as text."""
    a, b = counts(ours), counts(pin)
    lines = [f"instructions: ours {len(ours)}, pin {len(pin)}", "class    ours  pin  ours-pin"]
    lines += [f"{name:8} {a[name]:4} {b[name]:4} {a[name] - b[name]:+5}" for name in CLASSES]
    shown = 0
    for tag, i1, i2, j1, j2 in hunks(ours, pin):
        ours_part, pin_part = ours[i1:i2], pin[j1:j2]
        mine, theirs = counts(ours_part), counts(pin_part)
        if all(mine[name] == theirs[name] for name in CLASSES):
            continue
        shown += 1
        if shown > limit:
            break
        change = ", ".join(f"{name} {mine[name] - theirs[name]:+d}" for name in CLASSES if mine[name] != theirs[name])
        lines += ["", f"hunk {shown} ({tag}) at ours {i1}, pin {j1}: {change}"]
        lines += [f"  before: {one}" for one in ours[max(0, i1 - context) : i1]]
        lines += [f"  ours  : {one}" for one in ours_part] or ["  ours  : (none)"]
        lines += [f"  pin   : {one}" for one in pin_part] or ["  pin   : (none)"]
    if not shown:
        lines += ["", "no hunk changes the count of a class"]
    return "\n".join(lines)


def main() -> int:
    ask = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ask.add_argument("ours", type=Path)
    ask.add_argument("pin", type=Path)
    ask.add_argument("--context", type=int, default=2)
    ask.add_argument("--max", type=int, default=12)
    args = ask.parse_args()
    print(report(instructions(args.ours.read_text()), instructions(args.pin.read_text()), args.context, args.max))
    return 0


if __name__ == "__main__":
    sys.exit(main())
