#!/usr/bin/env python3
"""
The lift: BC's 16-bit instruction pairs read back as 32-bit values.

This is the step that turns instructions into meaning. BC keeps a long in
one of two register pairs, ax:dx or cx:bx, and every operation on it comes
out as two instructions -- the low half, then the high half with the carry
aware partner where there is one. Read as a pair, each is one 32-bit
operation on one value:

    mov cx,[X]   mov bx,[X+2]      v0 = load X
    and cx,[Y]   and bx,[Y+2]      v1 = v0 and Y
    mov dx,bx    mov ax,cx         a move -- ax:dx now holds v1
    xor ax,cx    xor dx,bx         v2 = v1 xor <whatever cx:bx holds>
    mov [W],ax   mov [W+2],dx      store v1 to W

Once the code is values rather than instructions the copies and the
reloads are visible as what they are, which is the whole point: matching
instruction patterns can never see that a pair-to-pair copy is dead.

Anything not recognised invalidates both pairs, because an instruction
this does not understand may write either of them.
"""

import struct
from enum import StrEnum
from dataclasses import dataclass

# the five operations, low half -> (high half, widened)
PAIRS = {
    0x23: (0x23, 0x23, "and"),
    0x0B: (0x0B, 0x0B, "or"),
    0x33: (0x33, 0x33, "xor"),
    0x03: (0x13, 0x03, "add"),
    0x2B: (0x1B, 0x2B, "sub"),
}
PAIR_OPCODES = {op for low, (high, _, _) in PAIRS.items() for op in (low, high)}

# reg field -> (pair, half)   0 ax, 1 cx, 2 dx, 3 bx
REG = {0: (0, 0), 1: (1, 0), 2: (0, 1), 3: (1, 1)}


class Kind(StrEnum):
    LOAD = "ld"
    STORE = "st"
    ALU = "op"
    MOVE = "mv"
    REG_ALU = "rr"


@dataclass(frozen=True, slots=True)
class Decoded:
    kind: Kind
    pair: int
    # half is always 0 or 1, never absent: A1/A3 are pair 0's low half, and
    # reporting "no half" for them makes the pairing test compare against None
    # and a store silently fail to match its own high half.
    half: int
    length: int
    src_pair: int = 0
    alu: int | None = None
    mem: int | None = None
    base: int = 0
    dlen: int = 0


class Op(StrEnum):
    LOAD = "load"
    ALUM = "alu-m"
    ALUV = "alu-v"
    STORE = "store"
    MOVE = "move"
    NEG = "neg"


@dataclass(slots=True)
class Value:
    op: Op
    at: int
    end: int
    alu: str | None = None  # the mnemonic, which is what OPC is keyed on
    s1: int | None = None
    s2: int | None = None
    mem: int | None = None
    pair: int = 0
    src_pair: int = 0
    base: int = 0x06
    dlen: int = 2

    def __repr__(self) -> str:
        match self.op:
            case Op.LOAD:
                return f"load [{self.mem:#06x}]"
            case Op.ALUM:
                return f"v{self.s1} {self.alu} [{self.mem:#06x}]"
            case Op.ALUV:
                return f"v{self.s1} {self.alu} v{self.s2}"
            case Op.MOVE:
                return f"move v{self.s1}"
            case Op.NEG:
                return f"neg v{self.s1}"
            case Op.STORE:
                return f"store v{self.s1} -> [{self.mem:#06x}]"
            case _:
                return str(self.op)


def memory_operand(code: bytes, at: int, base: int) -> tuple[int, int] | None:
    """The displacement and its width, for the ModRM forms BC uses for a long.

    A bare displacement for a static, bp-relative for a local or a spilled
    temporary. Both are carried through unchanged: the widened instruction
    keeps the same ModRM and displacement and changes only the register field,
    so nothing here has to understand what the address means."""
    match base:
        case 0x06 if at + 2 <= len(code):
            return struct.unpack_from("<H", code, at)[0], 2
        case 0x46 if at + 1 <= len(code):
            return struct.unpack_from("<b", code, at)[0], 1
        case 0x86 if at + 2 <= len(code):
            return struct.unpack_from("<h", code, at)[0], 2
        case _:
            return None


def classify(code: bytes, at: int) -> Decoded | None:
    """What the one instruction at `at` is, in long terms, or None."""
    # lift probes for the second half of a pair without knowing whether there
    # is one, so running off the end is ordinary and answers "not a pair"
    # rather than raising.
    if at >= len(code):
        return None
    opcode = code[at]

    # A1 and A3 are ax with no ModRM at all
    if opcode in (0xA1, 0xA3):
        if at + 3 > len(code):
            return None
        kind = Kind.LOAD if opcode == 0xA1 else Kind.STORE
        disp = struct.unpack_from("<H", code, at + 1)[0]
        return Decoded(kind, pair=0, half=0, length=3, mem=disp, base=0x06, dlen=2)

    if at + 1 >= len(code):
        return None
    modrm = code[at + 1]
    reg = (modrm >> 3) & 7
    if reg > 3:
        return None
    pair, half = REG[reg]
    base = modrm & 0xC7

    if (operand := memory_operand(code, at + 2, base)) is not None:
        disp, dlen = operand
        length = 2 + dlen
        if at + length > len(code):
            return None
        match opcode:
            case 0x8B:
                kind = Kind.LOAD
            case 0x89:
                kind = Kind.STORE
            case _ if opcode in PAIR_OPCODES:
                kind = Kind.ALU
            case _:
                return None
        alu = opcode if kind is Kind.ALU else None
        return Decoded(kind, pair, half, length, alu=alu, mem=disp, base=base, dlen=dlen)

    if modrm & 0xC0 != 0xC0:  # register to register
        return None
    rm = modrm & 7
    if rm > 3:
        return None
    src_pair, src_half = REG[rm]
    if src_half != half:
        return None  # halves must match
    match opcode:
        case 0x8B:
            return Decoded(Kind.MOVE, pair, half, length=2, src_pair=src_pair)
        case _ if opcode in PAIR_OPCODES:
            return Decoded(Kind.REG_ALU, pair, half, length=2, src_pair=src_pair, alu=opcode)
        case _:
            return None


# neg lo / adc hi,0 / neg hi, per pair. Three instructions, so it does not fit
# the pair shape at all -- and not recognising it is expensive out of all
# proportion to how often it appears, because it invalidates both pairs and
# every operation after it until the next load then falls out too.
NEGATE = {0: bytes([0xF7, 0xD8, 0x83, 0xD2, 0x00, 0xF7, 0xDA]), 1: bytes([0xF7, 0xD9, 0x83, 0xD3, 0x00, 0xF7, 0xDB])}


def widened(
    op: Op,
    at: int,
    span: int,
    shape: Decoded,
    mem: int | None,
    source: int | None = None,
    alu: str | None = None,
) -> Value:
    """One value standing for the instruction pair at `at`."""
    return Value(
        op,
        at=at,
        end=at + span,
        pair=shape.pair,
        base=shape.base,
        dlen=shape.dlen,
        mem=mem,
        s1=source,
        alu=alu,
    )


def lift(code: bytes, start: int, end: int) -> tuple[list[Value], list[int]]:
    """Values, stores, and the instructions consumed, for one straight run."""
    values: list[Value] = []
    stores: list[int] = []
    live: dict[int, int | None] = {0: None, 1: None}
    at = start

    def add(value: Value) -> int:
        values.append(value)
        return len(values) - 1

    while at < end:
        for pair, pattern in NEGATE.items():
            if live[pair] is not None and code[at : at + 7] == pattern and at + 7 <= end:
                live[pair] = add(Value(Op.NEG, s1=live[pair], at=at, end=at + 7, pair=pair))
                at += 7
                break
        if at >= end:
            break

        first = classify(code, at)
        if first is None:
            live[0] = live[1] = None
            at += 1
            continue
        second = classify(code, at + first.length)

        paired = False
        if second is not None and (
            second.kind == first.kind
            and second.pair == first.pair
            and second.src_pair == first.src_pair
            and second.half != first.half
            and second.base == first.base
        ):
            span = first.length + second.length
            low, high = (first, second) if first.half == 0 else (second, first)
            operation = PAIRS.get(low.alu) if low.alu is not None else None
            paired = low.mem is not None and high.mem == low.mem + 2
            if paired and first.kind is Kind.ALU:
                paired = operation is not None and operation[0] == high.alu

            if paired:
                match first.kind:
                    case Kind.LOAD:
                        live[first.pair] = add(widened(Op.LOAD, at, span, first, low.mem))
                    case Kind.ALU if live[first.pair] is not None and operation is not None:
                        live[first.pair] = add(
                            widened(Op.ALUM, at, span, first, low.mem, source=live[first.pair], alu=operation[2])
                        )
                    case Kind.STORE if live[first.pair] is not None:
                        stores.append(add(widened(Op.STORE, at, span, first, low.mem, source=live[first.pair])))
                    case _:
                        paired = False

            elif first.kind is Kind.MOVE and live[first.src_pair] is not None:
                # A move is a value, not nothing. Dropping it looks tempting --
                # the halves are just being copied -- but the source pair is
                # usually reused straight afterwards, so the copy is what keeps
                # the value alive. And it cannot be left as BC wrote it either:
                # mov ax,cx writes sixteen bits and leaves the top half of eax
                # stale, which a widened store then writes out as garbage. So it
                # becomes one 32-bit move, three bytes against four.
                live[first.pair] = add(
                    Value(
                        Op.MOVE,
                        s1=live[first.src_pair],
                        at=at,
                        end=at + span,
                        pair=first.pair,
                        src_pair=first.src_pair,
                    )
                )
                paired = True

            elif (
                first.kind is Kind.REG_ALU
                and operation is not None
                and live[first.pair] is not None
                and live[first.src_pair] is not None
            ):
                live[first.pair] = add(
                    Value(
                        Op.ALUV,
                        alu=operation[2],
                        s1=live[first.pair],
                        s2=live[first.src_pair],
                        at=at,
                        end=at + span,
                        pair=first.pair,
                        src_pair=first.src_pair,
                    )
                )
                paired = True

            if paired:
                at += span
                continue

        live[0] = live[1] = None
        at += 1

    return values, stores


# ---------------------------------------------------------------------------
# From the graph to code.
#
# A value lives in the register pair BC computed it in, and stays there. That
# is the register allocation, and it is nearly free: BC already did it, and
# inheriting its choice means the operations need no moves between registers.
# What it buys is what BC could not express in 16 bits -- a pair-to-pair copy
# becomes a rebinding rather than two instructions, and a value fed to another
# pair is one instruction rather than two.


def regions(values: list[Value]) -> list[list[int]]:
    """Maximal runs of values whose instructions are contiguous in the code.
    Anything unlifted between two values ends a region -- it has to stay
    where it is, so the rewrite cannot span it."""
    found: list[list[int]] = []
    current: list[int] = []
    for index, value in enumerate(values):
        if current and values[current[-1]].end != value.at:
            found.append(current)
            current = []
        current.append(index)
    if current:
        found.append(current)
    return found


def needed(values: list[Value], within: list[list[int]] | None = None) -> list[bool]:
    """Which values have to be computed.

    Stores write memory, so they always count. And whatever is left in a
    register when a *region* ends counts too: the code after the region was
    not lifted, so there is no telling what it reads, and BC routinely opens
    the next statement on the value the last one left in the pair. Asking
    this globally instead of per region drops exactly those values, and the
    statement that follows then reads one that was never computed."""
    need = [value.op is Op.STORE for value in values]
    for region in within if within is not None else regions(values):
        last_in_pair = {values[i].pair: i for i in region if values[i].op is not Op.STORE}
        for index in last_in_pair.values():
            need[index] = True
    for index in reversed(range(len(values))):
        if need[index]:
            for source in (values[index].s1, values[index].s2):
                if source is not None:
                    need[source] = True
    return need


def sizeof(value: Value) -> int:
    return len(encode(value))


OPC = {"and": 0x23, "or": 0x0B, "xor": 0x33, "add": 0x03, "sub": 0x2B}
LOREG = {0: 0, 1: 1}  # pair 0 low is eax (000), pair 1 is ecx (001)


def displacement(value: Value) -> bytes:
    if value.mem is None:
        return b""
    if value.dlen == 1:
        return struct.pack("<b", value.mem)
    return struct.pack("<h" if value.mem < 0 else "<H", value.mem)


def encode(value: Value) -> bytes:
    """The widened form of one value. Register is the pair BC used."""
    opcode = OPC.get(value.alu, 0)
    disp = displacement(value)
    from_memory = value.base | (LOREG[value.pair] << 3)
    # mod 11, reg = destination pair's low register, rm = source's
    from_register = 0xC0 | (LOREG[value.pair] << 3) | LOREG[value.src_pair]
    match value:
        case Value(op=Op.LOAD, pair=0, base=0x06):
            return bytes([0x66, 0xA1]) + disp
        case Value(op=Op.LOAD):
            return bytes([0x66, 0x8B, from_memory]) + disp
        case Value(op=Op.STORE, pair=0, base=0x06):
            return bytes([0x66, 0xA3]) + disp
        case Value(op=Op.STORE):
            return bytes([0x66, 0x89, from_memory]) + disp
        case Value(op=Op.ALUM):
            return bytes([0x66, opcode, from_memory]) + disp
        case Value(op=Op.ALUV):
            return bytes([0x66, opcode, from_register])
        case Value(op=Op.MOVE):
            return bytes([0x66, 0x8B, from_register])
        case Value(op=Op.NEG):
            return bytes([0x66, 0xF7, 0xD8 | LOREG[value.pair]])
        case _:
            raise ValueError(f"no widened form for {value.op}")


# Putting the high half back. The widened form writes only the 32-bit
# register, so dx and bx are left holding whatever they held before the
# region -- and BC reads them, because in its world that is where the top
# half of the long lives. Without liveness across blocks there is no telling
# whether anything after the region wants them, so they go back.
FIXUP = {
    0: bytes([0x66, 0x8B, 0xD0, 0x66, 0xC1, 0xEA, 0x10]),  # edx=eax, shr 16
    1: bytes([0x66, 0x8B, 0xD9, 0x66, 0xC1, 0xEB, 0x10]),
}  # ebx=ecx, shr 16


def emit_region(values: list[Value], need: list[bool], region: list[int]) -> bytes | None:
    """Bytes for one region, plus the jump over whatever is left."""
    live = [i for i in region if need[i]]
    out = b"".join(encode(values[i]) for i in live)
    out += b"".join(FIXUP[pair] for pair in sorted({values[i].pair for i in live}))

    slack = (values[region[-1]].end - values[region[0]].at) - len(out)
    match slack:
        case _ if slack < 0:
            return None
        case 0:
            return out
        case 1:
            return out + b"\x90"
        case _:
            return out + bytes([0xEB, slack - 2]) + b"\x90" * (slack - 2)
