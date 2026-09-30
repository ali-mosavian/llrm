"""
MSP430 instructions from their bytes: enough of them for innerloops.py to
find loops in LLVM's msp430 objects and count their instructions, memory
operands and stepped registers. MSP430 (not 430X), as clang emits by default.
"""

from dataclasses import dataclass, field

import iced_x86 as ix

JUMPS = ("jne", "jeq", "jnc", "jc", "jn", "jge", "jl", "jmp")
SINGLE = ("rrc", "swpb", "rra", "sxt", "push", "call", "reti", None)
DOUBLE = {4: "mov", 5: "add", 6: "addc", 7: "subc", 8: "sub", 9: "cmp", 10: "dadd", 11: "bit", 12: "bic", 13: "bis",
          14: "xor", 15: "and"}
REGS = ("pc", "sp", "sr", "cg") + tuple(f"r{n}" for n in range(4, 16))


@dataclass
class Insn:
    ip: int
    next_ip: int
    mnemonic: str
    text: str
    flow_control: int
    target: int | None = None
    memory: int = 0
    writes: set = field(default_factory=set)
    reads: set = field(default_factory=set)
    # register -> the constant it is stepped by, where that is all this does to it
    steps: dict = field(default_factory=dict)


def _word(code: bytes, at: int) -> int | None:
    return int.from_bytes(code[at : at + 2], "little") if at + 2 <= len(code) else None


def _source(code: bytes, at: int, mode: int, reg: int, byte: bool):
    """(text, extension bytes, is memory, constant or None, autoincrement)."""
    if mode == 0:
        if reg == 3:
            return "#0", 0, False, 0, False
        return REGS[reg], 0, False, None, False
    if mode == 1:
        if reg == 3:
            return "#1", 0, False, 1, False
        x = _word(code, at)
        if x is None:
            return None
        if reg == 2:
            return f"&0x{x:04x}", 2, True, None, False
        return f"{x}({REGS[reg]})", 2, True, None, False
    if mode == 2:
        if reg == 2:
            return "#4", 0, False, 4, False
        if reg == 3:
            return "#2", 0, False, 2, False
        return f"@{REGS[reg]}", 0, True, None, False
    if reg == 0:
        x = _word(code, at)
        if x is None:
            return None
        value = x - 0x10000 if x & 0x8000 else x
        return f"#{value}", 2, False, value, False
    if reg == 2:
        return "#8", 0, False, 8, False
    if reg == 3:
        return "#-1", 0, False, -1, False
    return f"@{REGS[reg]}+", 0, True, None, True


def decode(code: bytes, ip: int) -> Insn | None:
    word = _word(code, ip)
    if word is None:
        return None
    F = ix.FlowControl
    if word >> 13 == 1:
        condition = (word >> 10) & 7
        offset = word & 0x3FF
        offset = offset - 0x400 if offset & 0x200 else offset
        target = ip + 2 + 2 * offset
        name = JUMPS[condition]
        flow = F.UNCONDITIONAL_BRANCH if name == "jmp" else F.CONDITIONAL_BRANCH
        return Insn(ip, ip + 2, name, f"{name} 0x{target:04x}", flow, target, reads={"sr"} if name != "jmp" else set())
    if word >> 10 == 4:
        name = SINGLE[(word >> 7) & 7]
        if name is None:
            return None
        byte = bool(word & 0x40)
        mode, reg = (word >> 4) & 3, word & 15
        if name == "reti":
            return Insn(ip, ip + 2, name, name, F.RETURN)
        found = _source(code, ip + 2, mode, reg, byte)
        if found is None:
            return None
        text, extra, memory, constant, autoinc = found
        suffix = ".b" if byte else ""
        one = Insn(ip, ip + 2 + extra, name, f"{name}{suffix} {text}", F.NEXT, memory=int(memory))
        if name == "call":
            one.flow_control = F.CALL
            one.target = constant if constant is not None and mode == 3 else None
        elif mode == 0 and name != "push":
            one.writes.add(REGS[reg])
        if mode != 0 or name == "push":
            one.reads.add(REGS[reg])
        if name == "push":
            one.memory += 1
        if autoinc:
            one.writes.add(REGS[reg])
            one.steps[REGS[reg]] = 1 if byte else 2
        return one
    opcode = word >> 12
    if opcode not in DOUBLE:
        return None
    name = DOUBLE[opcode]
    source, ad, byte, mode, dest = (word >> 8) & 15, (word >> 7) & 1, bool(word & 0x40), (word >> 4) & 3, word & 15
    found = _source(code, ip + 2, mode, source, byte)
    if found is None:
        return None
    text, extra, memory, constant, autoinc = found
    at = ip + 2 + extra
    if ad:
        x = _word(code, at)
        if x is None:
            return None
        destination = f"&0x{x:04x}" if dest == 2 else f"{x}({REGS[dest]})"
        at += 2
    else:
        destination = REGS[dest]
    suffix = ".b" if byte else ""
    one = Insn(ip, at, name, f"{name}{suffix} {text}, {destination}", F.NEXT, memory=int(memory) + ad)
    if mode != 0 and source not in (2, 3) or (mode == 0 and source != 3):
        one.reads.add(REGS[source])
    if autoinc:
        one.writes.add(REGS[source])
        one.steps[REGS[source]] = 1 if byte else 2
    if ad:
        one.reads.add(REGS[dest])
    elif name not in ("cmp", "bit"):
        one.writes.add(REGS[dest])
        if name != "mov":
            one.reads.add(REGS[dest])
        if name in ("add", "sub") and constant is not None and REGS[dest] not in one.steps:
            one.steps[REGS[dest]] = constant if name == "add" else -constant
    else:
        one.reads.add(REGS[dest])
    if dest == 0 and not ad and name == "mov":
        if mode == 3 and source == 1:
            one.flow_control = F.RETURN
            one.text = "ret"
        else:
            one.flow_control = F.UNCONDITIONAL_BRANCH if constant is not None else F.INDIRECT_BRANCH
            one.target = constant & 0xFFFF if constant is not None else None
    return one
