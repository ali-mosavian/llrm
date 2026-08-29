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

from enum import StrEnum
from dataclasses import dataclass
from collections.abc import Callable

from iced_x86 import Code
from iced_x86 import Encoder
from iced_x86 import Register
from iced_x86 import Instruction
from iced_x86 import MemoryOperand

from qbopt.declen import run
from qbopt.flags import Flag
from qbopt.declen import Insn
from qbopt.module import Addr
from qbopt.module import Space
from qbopt.declen import BITNESS
from qbopt.flags import DIVERGENT
from qbopt.module import literal_only
from qbopt.module import frame_relative

# the five operations, low half -> (high half, widened)


class Kind(StrEnum):
    LOAD = "ld"
    STORE = "st"
    ALU = "op"
    MOVE = "mv"
    REG_ALU = "rr"
    NOT = "not"


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
    NOT = "not"


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


# The five operations, as iced names each half. BC does the low half with the
# plain form and the high half with the carry-aware partner where there is one.
PAIRED = {
    Code.AND_R16_RM16: (Code.AND_R16_RM16, "and"),
    Code.OR_R16_RM16: (Code.OR_R16_RM16, "or"),
    Code.XOR_R16_RM16: (Code.XOR_R16_RM16, "xor"),
    Code.ADD_R16_RM16: (Code.ADC_R16_RM16, "add"),
    Code.SUB_R16_RM16: (Code.SBB_R16_RM16, "sub"),
}
HIGH_HALVES = {high for high, _name in PAIRED.values()}

LOADS = {Code.MOV_R16_RM16, Code.MOV_AX_MOFFS16}
STORES = {Code.MOV_RM16_R16, Code.MOV_MOFFS16_AX}

# reg -> (pair, half). ax:dx is pair 0, cx:bx is pair 1.
HALF_OF = {
    Register.AX: (0, 0),
    Register.DX: (0, 1),
    Register.CX: (1, 0),
    Register.BX: (1, 1),
}

type Resolver = Callable[[int, int], Addr]


def operand(insn: Insn, resolve: Resolver) -> Addr | None:
    """Where this instruction's memory operand points, or None if it has none.

    A bare displacement is a static, and its address is not in the code: the
    field holds zero and the fixup names the target. bp-relative is a local or a
    spilled temporary, and its displacement really is in the code.
    """
    if insn.disp_at is None:
        return None
    match insn.memory_base:
        case Register.NONE:
            return resolve(insn.disp_at, insn.insn.memory_displacement)
        case Register.BP:
            return frame_relative(insn.displacement)
        case _:
            return None


def classify(insn: Insn, resolve: Resolver = literal_only) -> Decoded | None:
    """What one instruction is, in long terms, or None.

    Everything here is a 16-bit form. iced puts the operand size in the code, so
    the widened forms this pass emits cannot be mistaken for a half of anything.
    """
    code = insn.code
    if code in LOADS or code in STORES:
        register = insn.register(0 if code in LOADS else 1)
        if register not in HALF_OF:
            return None
        pair, half = HALF_OF[register]
        kind = Kind.LOAD if code in LOADS else Kind.STORE
        if not insn.reads_memory(1 if code in LOADS else 0):
            # register to register: a pair copy, if the halves line up
            source = insn.register(1 if code in LOADS else 0)
            if code not in LOADS or source not in HALF_OF or HALF_OF[source][1] != half:
                return None
            return Decoded(Kind.MOVE, pair, half, insn.length, src_pair=HALF_OF[source][0])
        where = operand(insn, resolve)
        if where is None:
            return None
        return Decoded(kind, pair, half, insn.length, mem=where, dlen=insn.disp_len, disp_at=insn.disp_at)

    if code == Code.NOT_RM16 and not insn.reads_memory(0):
        # NOT, EQV and IMP all go through this pair, and it writes no flag
        register = insn.register(0)
        if register not in HALF_OF:
            return None
        pair, half = HALF_OF[register]
        return Decoded(Kind.NOT, pair, half, insn.length)

    if code not in PAIRED and code not in HIGH_HALVES:
        return None
    register = insn.register(0)
    if register not in HALF_OF:
        return None
    pair, half = HALF_OF[register]
    if not insn.reads_memory(1):
        source = insn.register(1)
        if source not in HALF_OF or HALF_OF[source][1] != half:
            return None
        return Decoded(Kind.REG_ALU, pair, half, insn.length, src_pair=HALF_OF[source][0], alu=code)
    where = operand(insn, resolve)
    if where is None:
        return None
    return Decoded(Kind.ALU, pair, half, insn.length, alu=code, mem=where, dlen=insn.disp_len, disp_at=insn.disp_at)


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
            operation = PAIRED.get(low.alu) if low.alu is not None else None
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
                                alu=operation[1],
                            )
                        )
                    case Kind.STORE if live[first.pair] is not None:
                        stores.append(
                            add(widened(Op.STORE, at, span, first, low.mem, low.disp_at, source=live[first.pair]))
                        )
                    case _:
                        paired = False

            elif first.kind is Kind.NOT and live[first.pair] is not None:
                live[first.pair] = add(widened(Op.NOT, at, span, first, None, source=live[first.pair]))
                paired = True

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
                        alu=operation[1],
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


@dataclass(frozen=True, slots=True)
class Emitted:
    code: bytes
    # (offset within `code`, where the operand's field was) for each displacement
    # that has to be relocated. The fixup BC wrote for that field is reused
    # rather than a new one constructed: it already names the right target, and
    # copying it keeps the diff a short list.
    relocations: tuple[tuple[int, int], ...] = ()


# The widened form of each operation, and the register each pair widens into.
WIDE = {
    "and": Code.AND_R32_RM32,
    "or": Code.OR_R32_RM32,
    "xor": Code.XOR_R32_RM32,
    "add": Code.ADD_R32_RM32,
    "sub": Code.SUB_R32_RM32,
}
WIDE_REGISTER = {0: Register.EAX, 1: Register.ECX}


def memory(value: Value) -> MemoryOperand:
    """The operand as the widened instruction has to carry it.

    A relocated address is emitted as zero: LINK adds what is in the code to the
    fixup's target, so anything else would be added to the real address.
    """
    match value.mem:
        case Addr(space=Space.SEGMENT):
            return MemoryOperand(displ=0, displ_size=2)
        case Addr(space=Space.FRAME, disp=disp):
            return MemoryOperand(base=Register.BP, displ=disp, displ_size=value.dlen or 1)
        case Addr(disp=disp):
            return MemoryOperand(displ=disp, displ_size=2)
        case _:
            raise ValueError(f"{value.op} has no memory operand")


def instruction(value: Value) -> Instruction:
    """The one 386 instruction this value becomes."""
    wide = WIDE_REGISTER[value.pair]
    source = WIDE_REGISTER[value.src_pair]
    # pair 0 is eax, so a bare displacement takes the shorter moffs form
    moffs = value.pair == 0 and value.mem is not None and value.mem.space is not Space.FRAME
    match value.op:
        case Op.LOAD if moffs:
            return Instruction.create_reg_mem(Code.MOV_EAX_MOFFS32, wide, memory(value))
        case Op.LOAD:
            return Instruction.create_reg_mem(Code.MOV_R32_RM32, wide, memory(value))
        case Op.STORE if moffs:
            return Instruction.create_mem_reg(Code.MOV_MOFFS32_EAX, memory(value), wide)
        case Op.STORE:
            return Instruction.create_mem_reg(Code.MOV_RM32_R32, memory(value), wide)
        case Op.ALUM if value.alu is not None:
            return Instruction.create_reg_mem(WIDE[value.alu], wide, memory(value))
        case Op.ALUV if value.alu is not None:
            return Instruction.create_reg_reg(WIDE[value.alu], wide, source)
        case Op.MOVE:
            return Instruction.create_reg_reg(Code.MOV_R32_RM32, wide, source)
        case Op.NEG:
            return Instruction.create_reg(Code.NEG_RM32, wide)
        case Op.NOT:
            return Instruction.create_reg(Code.NOT_RM32, wide)
        case _:
            raise ValueError(f"no widened form for {value.op}")


def emit(value: Value) -> Emitted:
    """The widened form, and where it needs a fixup of its own."""
    encoder = Encoder(BITNESS)
    encoder.encode(instruction(value), 0)
    where = encoder.get_constant_offsets()
    code = encoder.take_buffer()

    if value.mem is None or value.mem.space is not Space.SEGMENT or value.mem_at is None:
        return Emitted(code)
    return Emitted(code, ((where.displacement_offset, value.mem_at),))


def encode(value: Value) -> bytes:
    return emit(value).code


def sizeof(value: Value) -> int:
    return len(encode(value))


# Putting the high half back. The widened form writes only the 32-bit register,
# so dx and bx are left holding whatever they held before the region -- and BC
# reads them, because in its world that is where the top half of the long lives.
# Without liveness across blocks there is no telling whether anything after the
# region wants them, so they go back.
#
# Through the stack rather than a shift: four bytes against seven, and it writes
# no flags at all, where `shr` wrote five of the six and made a region that
# computes nothing still destroy what followed it. tests/test_flags.py asserts
# that transparency -- if this ever changes, the gate needs the destruction mask
# back.
FIXUP = {
    0: bytes([0x66, 0x50, 0x58, 0x5A]),  # push eax / pop ax / pop dx
    1: bytes([0x66, 0x51, 0x59, 0x5B]),  # push ecx / pop cx / pop bx
}


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
    return Emitted(bytes(out + restore), tuple(relocations))
