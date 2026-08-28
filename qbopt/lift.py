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
from collections.abc import Callable

from qbopt.flags import ALL
from qbopt.declen import run
from qbopt.flags import Flag
from qbopt.declen import Insn
from qbopt.module import Addr
from qbopt.module import Space
from qbopt.flags import DIVERGENT
from qbopt.declen import to_signed
from qbopt.module import literal_only
from qbopt.module import frame_relative

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
    mem: Addr | None = None
    base: int = 0
    dlen: int = 0
    # where the displacement field sat, so the fixup that named it can be found
    disp_at: int | None = None


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
    mem: Addr | None = None
    pair: int = 0
    src_pair: int = 0
    base: int = 0x06
    dlen: int = 2
    mem_at: int | None = None

    def __repr__(self) -> str:
        match self.op:
            case Op.LOAD:
                return f"load {self.mem}"
            case Op.ALUM:
                return f"v{self.s1} {self.alu} {self.mem}"
            case Op.ALUV:
                return f"v{self.s1} {self.alu} v{self.s2}"
            case Op.MOVE:
                return f"move v{self.s1}"
            case Op.NEG:
                return f"neg v{self.s1}"
            case Op.STORE:
                return f"store v{self.s1} -> {self.mem}"
            case _:
                return str(self.op)


# The memory forms BC uses for a long: a bare displacement for a static, and
# bp-relative for a local or a spilled temporary. Both are carried through
# unchanged -- the widened instruction keeps the same ModRM and displacement and
# changes only the register field, so nothing here has to understand what the
# address means.
STATIC = 0x06
BASES = (STATIC, 0x46, 0x86)  # [disp16], [bp+disp8], [bp+disp16]

type Resolver = Callable[[int, int], Addr]


def classify(insn: Insn, resolve: Resolver = literal_only) -> Decoded | None:
    """What one instruction is, in long terms, or None.

    `resolve` turns the offset of a displacement field, and whatever literal is
    sitting in it, into the address it really names. In an object that literal
    is zero and the address is in a fixup; in a unit test there is no fixup and
    the literal is the address.
    """
    if insn.prefixes:
        # A 66-prefixed instruction is already 32-bit, so it is not half of
        # anything. BC emits none inside a pair.
        return None

    # A1 and A3 are ax with no ModRM at all
    if insn.opcode in (0xA1, 0xA3) and insn.disp_at is not None:
        kind = Kind.LOAD if insn.opcode == 0xA1 else Kind.STORE
        mem = resolve(insn.disp_at, insn.disp or 0)
        return Decoded(kind, pair=0, half=0, length=insn.length, mem=mem, base=STATIC, dlen=2, disp_at=insn.disp_at)

    if insn.modrm is None or insn.reg > 3:
        return None
    pair, half = REG[insn.reg]
    base = insn.modrm & 0xC7

    if base in BASES and insn.disp_at is not None:
        mem = (
            resolve(insn.disp_at, insn.disp or 0)
            if base == STATIC
            else frame_relative(to_signed(insn.disp or 0, insn.disp_len))
        )
        match insn.opcode:
            case 0x8B:
                kind = Kind.LOAD
            case 0x89:
                kind = Kind.STORE
            case opcode if opcode in PAIR_OPCODES:
                kind = Kind.ALU
            case _:
                return None
        alu = insn.opcode if kind is Kind.ALU else None
        return Decoded(
            kind, pair, half, insn.length, alu=alu, mem=mem, base=base, dlen=insn.disp_len, disp_at=insn.disp_at
        )

    if insn.mod != 3:  # register to register
        return None
    if insn.rm > 3:
        return None
    src_pair, src_half = REG[insn.rm]
    if src_half != half:
        return None  # halves must match
    match insn.opcode:
        case 0x8B:
            return Decoded(Kind.MOVE, pair, half, insn.length, src_pair=src_pair)
        case opcode if opcode in PAIR_OPCODES:
            return Decoded(Kind.REG_ALU, pair, half, insn.length, src_pair=src_pair, alu=opcode)
        case _:
            return None


# neg lo / adc hi,0 / neg hi, per pair. Three instructions, so it does not fit
# the pair shape at all -- and not recognising it is expensive out of all
# proportion to how often it appears, because it invalidates both pairs and
# every operation after it until the next load then falls out too.
NEGATE = {0: bytes([0xF7, 0xD8, 0x83, 0xD2, 0x00, 0xF7, 0xDA]), 1: bytes([0xF7, 0xD9, 0x83, 0xD3, 0x00, 0xF7, 0xDB])}


def negate_at(code: bytes, at: int) -> int | None:
    """The pair a three-instruction long negate at `at` targets, or None."""
    for pair, pattern in NEGATE.items():
        if code[at : at + len(pattern)] == pattern:
            return pair
    return None


def widened(
    op: Op,
    at: int,
    span: int,
    shape: Decoded,
    mem: Addr | None,
    mem_at: int | None = None,
    source: int | None = None,
    second_source: int | None = None,
    alu: str | None = None,
) -> Value:
    """One value standing for the instruction pair at `at`."""
    return Value(
        op,
        at=at,
        end=at + span,
        pair=shape.pair,
        src_pair=shape.src_pair,
        base=shape.base,
        dlen=shape.dlen,
        mem=mem,
        mem_at=mem_at,
        s1=source,
        s2=second_source,
        alu=alu,
    )


def pairs_with(first: Decoded, second: Decoded) -> bool:
    """Whether two classified instructions are the two halves of one long."""
    return (
        second.kind == first.kind
        and second.pair == first.pair
        and second.src_pair == first.src_pair
        and second.half != first.half
        and second.base == first.base
    )


def lift(
    code: bytes,
    start: int,
    end: int,
    resolve: Resolver = literal_only,
    stream: list[Insn] | None = None,
) -> tuple[list[Value], list[int]]:
    """Values and stores, over the instructions given or a straight run of them.

    `stream` is what the block finder reached. Without it this decodes linearly,
    which is right for a hand-built test and wrong for a module: a linear walk
    reads jump tables and dead code as instructions, and a region built on one
    of those does not begin where an instruction does.
    """
    instructions = stream if stream is not None else run(code, start, end)[0]
    index_of = {insn.at: position for position, insn in enumerate(instructions)}

    values: list[Value] = []
    stores: list[int] = []
    live: dict[int, int | None] = {0: None, 1: None}

    def add(value: Value) -> int:
        values.append(value)
        return len(values) - 1

    position = 0
    while position < len(instructions):
        at = instructions[position].at

        pair = negate_at(code, at)
        # the negate is three instructions, and may be the last thing in the run
        after = index_of.get(at + 7, len(instructions) if at + 7 == end else None)
        if pair is not None and live[pair] is not None and after is not None:
            live[pair] = add(Value(Op.NEG, s1=live[pair], at=at, end=at + 7, pair=pair))
            position = after
            continue

        first = classify(instructions[position], resolve)
        second = classify(instructions[position + 1], resolve) if position + 1 < len(instructions) else None

        paired = False
        if first is not None and second is not None and pairs_with(first, second):
            span = instructions[position + 1].end - at
            low, high = (first, second) if first.half == 0 else (second, first)
            operation = PAIRS.get(low.alu) if low.alu is not None else None
            paired = low.mem is not None and high.mem == low.mem.plus(2)
            if paired and first.kind is Kind.ALU:
                paired = operation is not None and operation[0] == high.alu

            if paired:
                match first.kind:
                    case Kind.LOAD:
                        live[first.pair] = add(widened(Op.LOAD, at, span, first, low.mem, low.disp_at))
                    case Kind.ALU if live[first.pair] is not None and operation is not None:
                        live[first.pair] = add(
                            widened(
                                Op.ALUM,
                                at,
                                span,
                                first,
                                low.mem,
                                low.disp_at,
                                source=live[first.pair],
                                alu=operation[2],
                            )
                        )
                    case Kind.STORE if live[first.pair] is not None:
                        stores.append(
                            add(widened(Op.STORE, at, span, first, low.mem, low.disp_at, source=live[first.pair]))
                        )
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
                live[first.pair] = add(widened(Op.MOVE, at, span, first, None, source=live[first.src_pair]))
                paired = True

            elif (
                first.kind is Kind.REG_ALU
                and operation is not None
                and live[first.pair] is not None
                and live[first.src_pair] is not None
            ):
                live[first.pair] = add(
                    widened(
                        Op.ALUV,
                        at,
                        span,
                        first,
                        None,
                        source=live[first.pair],
                        second_source=live[first.src_pair],
                        alu=operation[2],
                    )
                )
                paired = True

        if paired:
            position += 2
            continue

        # An instruction this does not understand may have written either pair.
        # Advancing one byte instead of one instruction is how a matcher comes
        # to rewrite the middle of an instruction.
        live[0] = live[1] = None
        position += 1

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


OPC = {"and": 0x23, "or": 0x0B, "xor": 0x33, "add": 0x03, "sub": 0x2B}
LOREG = {0: 0, 1: 1}  # pair 0 low is eax (000), pair 1 is ecx (001)


def displacement(value: Value) -> bytes:
    match value.mem:
        case None:
            return b""
        case Addr(space=Space.SEGMENT):
            return b"\x00" * value.dlen
        case Addr(disp=disp) if value.dlen == 1:
            return struct.pack("<b", disp)
        case Addr(disp=disp):
            return struct.pack("<h" if disp < 0 else "<H", disp)
        case _:
            raise ValueError(f"no displacement for {value.mem}")


@dataclass(frozen=True, slots=True)
class Emitted:
    code: bytes
    # (offset within `code`, where the operand's field was) for each displacement
    # that has to be relocated. The fixup BC wrote for that field is reused
    # rather than a new one constructed: it already names the right target, and
    # copying it keeps the diff a short list.
    relocations: tuple[tuple[int, int], ...] = ()


def relocated_at(value: Value, code: bytes) -> tuple[tuple[int, int], ...]:
    if value.mem is None or value.mem.space is not Space.SEGMENT or value.mem_at is None:
        return ()
    return ((len(code) - value.dlen, value.mem_at),)


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


def sizeof(value: Value) -> int:
    return len(encode(value))


def emit(value: Value) -> Emitted:
    """The widened form, and where it needs a fixup of its own."""
    code = encode(value)
    return Emitted(code, relocated_at(value, code))


# Putting the high half back. The widened form writes only the 32-bit
# register, so dx and bx are left holding whatever they held before the
# region -- and BC reads them, because in its world that is where the top
# half of the long lives. Without liveness across blocks there is no telling
# whether anything after the region wants them, so they go back.
PUSHF = b"\x9c"
POPF = b"\x9d"

FIXUP = {
    0: bytes([0x66, 0x8B, 0xD0, 0x66, 0xC1, 0xEA, 0x10]),  # edx=eax, shr 16
    1: bytes([0x66, 0x8B, 0xD9, 0x66, 0xC1, 0xEB, 0x10]),
}  # ebx=ecx, shr 16


def computes(values: list[Value], need: list[bool], region: list[int]) -> bool:
    """Whether anything in the region leaves a flag whose value widening changes."""
    return any(values[i].op in (Op.ALUM, Op.ALUV, Op.NEG) for i in region if need[i])


def restored_pairs(values: list[Value], need: list[bool], region: list[int]) -> list[int]:
    """The register pairs whose high half has to be put back after the region."""
    return sorted({values[i].pair for i in region if need[i]})


def refuse(values: list[Value], need: list[bool], region: list[int], live: Flag) -> str | None:
    """Why this region cannot be rewritten, or None.

    BC leaves the high half's flags and one 32-bit operation leaves the whole
    result's, so where the region computes anything, ZF, PF and AF may come out
    different -- measured at 18.7, 37.2 and 12.4 per cent of cases. Nothing can
    be done about that except refuse, and a jz reading one of them afterwards is
    exactly the case that stays green until it does not.
    """
    if computes(values, need, region) and live & DIVERGENT:
        return f"widening would change {live & DIVERGENT!r} and something reads it"
    wanted = [values[i] for i in region if need[i]]
    if any(value.mem is not None and value.mem.space is Space.SEGMENT and value.mem_at is None for value in wanted):
        # A relocated operand's widened form holds zero, exactly as BC's does,
        # and the address comes from a fixup. Without knowing which field it
        # came from there is no fixup to reuse.
        return "a relocated operand has no field to take its fixup from"
    return None


def emit_region(values: list[Value], need: list[bool], region: list[int], live: Flag) -> Emitted | None:
    """The widened region, and the fixups it needs.

    `live` is not optional and has no default: the gate this parameter carries
    was designed into the predecessor and then lost in a refactor, and a default
    would make losing it again invisible.

    Nothing is padded. The runtime pass had to fit its rewrite into the bytes it
    replaced and jump over what it saved; here the code may move, so the region
    is exactly as long as it needs to be.
    """
    if refuse(values, need, region, live):
        return None

    out = bytearray()
    relocations: list[tuple[int, int]] = []
    for index in region:
        if not need[index]:
            continue
        one = emit(values[index])
        relocations += [(len(out) + at, field) for at, field in one.relocations]
        out += one.code

    restore = b"".join(FIXUP[pair] for pair in restored_pairs(values, need, region))
    if restore and live & ALL:
        # The shr in FIXUP writes every flag but AF, so a region that computes
        # nothing still destroys what follows it. Two bytes buy that back.
        restore = PUSHF + restore + POPF
    return Emitted(bytes(out + restore), tuple(relocations))
