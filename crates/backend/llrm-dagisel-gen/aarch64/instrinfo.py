#!/usr/bin/env python3
"""Reads the pin's <T>GenInstrInfo.inc (T = X86 or AArch64) (-gen-instr-info) into one line per instruction.

    instrinfo.py TGenInstrInfo.inc OUT.txt SYMBOLS.txt [T]

The opcode column of the pin's rows is not used: m16.td adds instructions and shifts every number
after them, so a row is named, and the name is looked up in the pin's headers or `own_ids.txt`.
Line: `name numops numdefs size flags uses defs op...` where flags is the MCID names joined by
`,` (or `-`), uses and defs are `X86::REG` names joined by `,` (or `-`), and an op is
`regclass|type|tied[/flags]` (regclass an `X86::` name or -1, type an MCOI operand type number or
an `X86::` target type, tied the operand it is tied to, `E` for an early clobber or `-`, flags `L` for
a pointer class looked up by hardware mode and `B` for a branch target). SYMBOLS.txt lists the X86:: names used.
"""
import re
import sys

src, out, symbols_out = sys.argv[1:4]
target = sys.argv[4] if len(sys.argv) > 4 else "X86"
text = open(src).read()

row = re.compile(r"^\s*\{ (\d+),\s*(\d+),\s*(\d+),\s*(\d+),\s*\d+,\s*(\d+),\s*(\d+),\s*" + target + r"OpInfoBase \+ (\d+),\s*(\d+),\s*(.*?),\s*0x[0-9a-fA-F]+ULL \},\s*// (\w+)$")

implicit_start = text.index("  }, {\n    /* 0 */")
implicit_end = text.index("  }, {", implicit_start + 5)
implicit = []
for line in text[implicit_start:implicit_end].splitlines():
    m = re.match(r"\s*/\* (\d+) \*/ (.*)", line)
    if not m:
        continue
    assert int(m.group(1)) == len(implicit), "implicit list offsets out of step"
    implicit.extend(r.strip() for r in m.group(2).split(",") if r.strip())

info_start = implicit_end
info_end = text.index("\n  }\n};", info_start)
operands = []
entry = re.compile(r"\{ (-?[\w:]+), ([^,]+), ([\w:]+), (0|MCOI_TIED_TO\((\d+)\)|MCOI_EARLY_CLOBBER) \}")
for line in text[info_start:info_end].splitlines():
    m = re.match(r"\s*/\* (\d+) \*/ (.*)", line)
    if not m:
        continue
    assert int(m.group(1)) == len(operands), "operand info offsets out of step"
    for e in entry.finditer(m.group(2)):
        cls, flags, kind, constraint, tied_op = e.groups()
        kind_num = {"MCOI::OPERAND_UNKNOWN": "0", "MCOI::OPERAND_IMMEDIATE": "1", "MCOI::OPERAND_REGISTER": "2",
                    "MCOI::OPERAND_MEMORY": "3", "MCOI::OPERAND_PCREL": "4"}.get(kind, kind)  # else a target type
        letters = ("L" if "LookupRegClassByHwMode" in flags else "") + ("B" if "BranchTarget" in flags else "")
        tied = tied_op if tied_op is not None else ("E" if constraint == "MCOI_EARLY_CLOBBER" else "-")
        operands.append((cls, kind_num, tied + ("/" + letters if letters else "")))

names = set()
with open(out, "w") as f:
    for line in text.splitlines():
        m = row.match(line)
        if not m:
            continue
        _opcode, numops, numdefs, size, nuses, ndefs, opoff, impoff, flags, name = m.groups()
        flag_names = re.findall(r"MCID::(\w+)", flags)
        regs = implicit[int(impoff):int(impoff) + int(nuses) + int(ndefs)]
        uses, defs = regs[: int(nuses)], regs[int(nuses):]
        names.update(uses + defs)
        ops = operands[int(opoff): int(opoff) + int(numops)]
        for cls, kind, _ in ops:
            names.update(n for n in (cls, kind) if n.startswith(target + "::"))
        op_text = " ".join(f"{c}|{k}|{t}" for c, k, t in ops)
        f.write(f"{name} {numops} {numdefs} {size} {','.join(flag_names) or '-'} "
                f"{','.join(uses) or '-'} {','.join(defs) or '-'} {op_text}\n")
with open(symbols_out, "w") as f:
    f.write("\n".join(sorted(n for n in names if n.startswith(target + "::"))) + "\n")
