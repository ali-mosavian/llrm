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
from dataclasses import replace
from dataclasses import dataclass
from collections.abc import Callable

from iced_x86 import Code
from iced_x86 import Encoder
from iced_x86 import Register
from iced_x86 import Register_
from iced_x86 import Instruction
from iced_x86 import MemoryOperand

from qbopt.declen import run
from qbopt.flags import Flag
from qbopt.declen import Insn
from qbopt.module import Addr
from qbopt.module import Space
from qbopt.declen import BITNESS
from qbopt.flags import DIVERGENT
from qbopt.declen import to_signed
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
    ALU_IMM = "opi"


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
    dlen: int = 0
    # where the displacement field sat, so the fixup that named it can be found
    disp_at: int | None = None
    # Kind.ALU_IMM only: this half's own 16-bit operand, already correct
    # regardless of which of the three immediate encodings (ax-implicit imm16,
    # rm16+imm16, rm16+imm8 sign-extended) BC picked -- iced's own immediate()
    # already reports the post-sign-extension value, so `& 0xFFFF` alone gives
    # the right 16 bits in every case (verified against real encoded bytes).
    imm: int | None = None


class Op(StrEnum):
    LOAD = "load"
    ALUM = "alu-m"
    ALUV = "alu-v"
    ALUI = "alu-i"
    STORE = "store"
    MOVE = "move"
    NEG = "neg"
    NOT = "not"
    CALL = "call"  # an absorbed call site (calls.py), already correctly
    # valued in pair 0 -- see RESTORE_EFFECTS' documented fact in ir.py, which
    # is the reason this needs no instruction of its own to "compute"


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
    dlen: int = 2
    mem_at: int | None = None
    imm: int | None = None  # Op.ALUI only: the combined 32-bit immediate, signed
    # Op.CALL only: calls.py's own already-assembled replacement for the call
    # site, restore=False (qbopt.calls.absorb) -- emit() returns this
    # verbatim rather than building an iced_x86.Instruction from the other
    # fields, which do not apply to a multi-instruction call absorption.
    absorbed: "Emitted | None" = None

    def __repr__(self) -> str:
        match self.op:
            case Op.LOAD:
                return f"load {self.mem}"
            case Op.ALUM:
                return f"v{self.s1} {self.alu} {self.mem}"
            case Op.ALUI:
                return f"v{self.s1} {self.alu} {self.imm}"
            case Op.ALUV:
                return f"v{self.s1} {self.alu} v{self.s2}"
            case Op.MOVE:
                return f"move v{self.s1}"
            case Op.NEG:
                return f"neg v{self.s1}"
            case Op.STORE:
                return f"store v{self.s1} -> {self.mem}"
            case Op.CALL:
                return "call result"
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

# The immediate-operand mirror of PAIRED: `add ax,imm16 / adc dx,imm8` and the
# and/or/xor/sub siblings. Three encodings exist per mnemonic because the
# assembler picks the shortest -- ax-implicit imm16, rm16+imm16, rm16+imm8
# sign-extended -- and BC chooses the low half's and the high half's encoding
# independently, so a low half may be any of the three and a high half any of
# its own family's three; they need not match each other's encoding. AND/OR/XOR
# have no carry variant, so (mirroring PAIRED's own AND/OR/XOR self-mapping)
# their own family is both the low and the high candidate set.
IMM_FAMILY: dict[int, str] = {
    Code.ADD_AX_IMM16: "add",
    Code.ADD_RM16_IMM16: "add",
    Code.ADD_RM16_IMM8: "add",
    Code.SUB_AX_IMM16: "sub",
    Code.SUB_RM16_IMM16: "sub",
    Code.SUB_RM16_IMM8: "sub",
    Code.AND_AX_IMM16: "and",
    Code.AND_RM16_IMM16: "and",
    Code.AND_RM16_IMM8: "and",
    Code.OR_AX_IMM16: "or",
    Code.OR_RM16_IMM16: "or",
    Code.OR_RM16_IMM8: "or",
    Code.XOR_AX_IMM16: "xor",
    Code.XOR_RM16_IMM16: "xor",
    Code.XOR_RM16_IMM8: "xor",
}
IMM_HIGH_FAMILY: dict[str, frozenset[int]] = {
    "add": frozenset({Code.ADC_AX_IMM16, Code.ADC_RM16_IMM16, Code.ADC_RM16_IMM8}),
    "sub": frozenset({Code.SBB_AX_IMM16, Code.SBB_RM16_IMM16, Code.SBB_RM16_IMM8}),
    "and": frozenset({Code.AND_AX_IMM16, Code.AND_RM16_IMM16, Code.AND_RM16_IMM8}),
    "or": frozenset({Code.OR_AX_IMM16, Code.OR_RM16_IMM16, Code.OR_RM16_IMM8}),
    "xor": frozenset({Code.XOR_AX_IMM16, Code.XOR_RM16_IMM16, Code.XOR_RM16_IMM8}),
}
IMM_HIGH_HALVES = frozenset().union(*IMM_HIGH_FAMILY.values())

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
    spilled temporary, and its displacement really is in the code. An array
    element is still a fixup-backed static -- the fixup names the array's own
    base -- but two elements at the same displacement are different addresses
    unless the register indexing them agrees too, so that register comes along.

    Refuses a segment-override prefix outright: this pass has no notion of a
    segment at all, and would otherwise conflate `es:[x]` with `ds:[x]`.
    Measured: zero instances in the reachable code of any of the 110 real
    fixtures, so this is a latent gap closed defensively, not a fix to an
    observed failure. Also refuses a Space.GROUP address -- see that space's
    own comment in module.py.
    """
    if insn.disp_at is None or insn.memory_index != Register.NONE or insn.has_segment_override:
        return None
    match insn.memory_base:
        case Register.NONE:
            resolved = resolve(insn.disp_at, insn.insn.memory_displacement)
            return None if resolved.space is Space.GROUP else resolved
        case Register.BP:
            return frame_relative(insn.displacement)
        case Register.SI | Register.DI:
            resolved = resolve(insn.disp_at, insn.insn.memory_displacement)
            return None if resolved.space is Space.GROUP else replace(resolved, base=insn.memory_base)
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

    if code in IMM_FAMILY or code in IMM_HIGH_HALVES:
        register = insn.register(0)
        if register not in HALF_OF:
            return None
        pair, half = HALF_OF[register]
        imm = insn.insn.immediate(1) & 0xFFFF
        return Decoded(Kind.ALU_IMM, pair, half, insn.length, alu=code, imm=imm)

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
    imm: int | None = None,
) -> Value:
    """One value standing for the instruction pair at `at`."""
    return Value(
        op,
        at=at,
        end=at + span,
        pair=shape.pair,
        src_pair=shape.src_pair,
        dlen=shape.dlen,
        mem=mem,
        mem_at=mem_at,
        s1=source,
        s2=second_source,
        alu=alu,
        imm=imm,
    )


def pairs_with(first: Decoded, second: Decoded) -> bool:
    """Whether two classified instructions are the two halves of one long."""
    return (
        second.kind == first.kind
        and second.pair == first.pair
        and second.src_pair == first.src_pair
        and second.half != first.half
    )


def _negate_step(
    code: bytes,
    instructions: list[Insn],
    index_of: dict[int, int],
    position: int,
    bound: int,
    live: dict[int, int | None],
    add: Callable[[Value], int],
) -> int | None:
    """Tries the three-instruction NEGATE idiom at `instructions[position]`.
    The position past it, or None if it does not apply here. Shared by
    lift()'s own walk and tail()'s, so a NEGATE right after a restored call
    (docs/residue.md's own worked G/H example) is recognised the same way a
    NEGATE anywhere else in a region already is -- one fact, one place."""
    at = instructions[position].at
    pair = negate_at(code, at)
    # the negate is three instructions, and may be the last thing in the run
    after = index_of.get(at + 7, len(instructions) if at + 7 == bound else None)
    if pair is not None and live[pair] is not None and after is not None:
        live[pair] = add(Value(Op.NEG, s1=live[pair], at=at, end=at + 7, pair=pair))
        return after
    return None


def _pair_step(
    instructions: list[Insn],
    position: int,
    resolve: Resolver,
    live: dict[int, int | None],
    add: Callable[[Value], int],
) -> int | None:
    """Tries a classify()-recognised instruction pair at `instructions[position]`.
    The position past it (`position + 2`), or None if nothing paired."""
    at = instructions[position].at
    first = classify(instructions[position], resolve)
    second = classify(instructions[position + 1], resolve) if position + 1 < len(instructions) else None
    if first is None or second is None or not pairs_with(first, second):
        return None

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
                    widened(Op.ALUM, at, span, first, low.mem, low.disp_at, source=live[first.pair], alu=operation[1])
                )
            case Kind.STORE if live[first.pair] is not None:
                add(widened(Op.STORE, at, span, first, low.mem, low.disp_at, source=live[first.pair]))
            case _:
                paired = False

    elif first.kind is Kind.NOT and live[first.pair] is not None:
        live[first.pair] = add(widened(Op.NOT, at, span, first, None, source=live[first.pair]))
        paired = True

    elif first.kind is Kind.MOVE and live[first.src_pair] is not None:
        # A move is a value, not nothing. Dropping it looks tempting -- the
        # halves are just being copied -- but the source pair is usually
        # reused straight afterwards, so the copy is what keeps the value
        # alive. And it cannot be left as BC wrote it either: mov ax,cx
        # writes sixteen bits and leaves the top half of eax stale, which a
        # widened store then writes out as garbage. So it becomes one 32-bit
        # move, three bytes against four.
        live[first.pair] = add(widened(Op.MOVE, at, span, first, None, source=live[first.src_pair]))
        paired = True

    elif (
        first.kind is Kind.ALU_IMM
        and low.alu in IMM_FAMILY
        and high.alu in IMM_HIGH_FAMILY[IMM_FAMILY[low.alu]]
        and low.imm is not None
        and high.imm is not None
        and live[first.pair] is not None
    ):
        # low.imm/high.imm are each already the correct 16-bit pattern
        # regardless of which of the three immediate encodings BC picked
        # (classify()'s own doc); combining and re-signing to a genuine
        # 32-bit int is what instruction()'s own encoding choice needs --
        # iced's builders reject an out-of-range unsigned pattern outright.
        combined = to_signed((high.imm << 16) | (low.imm & 0xFFFF), 4)
        name = IMM_FAMILY[low.alu]
        live[first.pair] = add(widened(Op.ALUI, at, span, first, None, source=live[first.pair], alu=name, imm=combined))
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

    return position + 2 if paired else None


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
        before = len(values)
        new_position = _negate_step(code, instructions, index_of, position, end, live, add)
        if new_position is None:
            new_position = _pair_step(instructions, position, resolve, live, add)
        if new_position is not None:
            if len(values) > before and values[-1].op is Op.STORE:
                stores.append(len(values) - 1)
            position = new_position
            continue

        # An instruction this does not understand may have written either pair.
        # Advancing one byte instead of one instruction is how a matcher comes
        # to rewrite the middle of an instruction.
        live[0] = live[1] = None
        position += 1

    return values, stores


def tail(instructions: list[Insn], code: bytes, bound: int, resolve: Resolver, seed: Value) -> list[Value]:
    """The maximal widenable run starting right after an absorbed call site,
    seeded with `seed` already correctly valued in pair 0 -- `calls.py`'s own
    restore idiom, byte-identical to `ir.RESTORE_EFFECTS[0]`, is exactly this
    fact: the call's real 32-bit result never left eax. Reuses lift()'s own
    pairing rules (`_negate_step`/`_pair_step`) unchanged, so the NEGATE
    idiom -- docs/residue.md's own largest single G/H example -- is covered
    without new logic.

    Unlike lift()'s own walk, this never falls through to "invalidate and
    keep scanning" on the first unrecognised instruction: there is nothing to
    resume a call site's tail into, and continuing scanning risks the tail
    swallowing an unrelated, unwidenable statement for no benefit.
    """
    index_of = {insn.at: position for position, insn in enumerate(instructions)}
    values: list[Value] = [seed]
    live: dict[int, int | None] = {0: 0, 1: None}

    def add(value: Value) -> int:
        values.append(value)
        return len(values) - 1

    position = 0
    while position < len(instructions):
        new_position = _negate_step(code, instructions, index_of, position, bound, live, add)
        if new_position is None:
            new_position = _pair_step(instructions, position, resolve, live, add)
        if new_position is None:
            break
        position = new_position

    return values


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

# Op.ALUI's three candidate encodings, shortest first: rm32,imm8 (sign-extended,
# 4 bytes with the 16-bit segment's own 0x66 prefix) whenever the combined value
# fits a signed byte; eax,imm32 (6 bytes, no ModRM -- pair 0 only, and shorter
# than the general rm32,imm32 form only because it has none); rm32,imm32 (7
# bytes) otherwise.
WIDE_IMM8 = {
    "and": Code.AND_RM32_IMM8,
    "or": Code.OR_RM32_IMM8,
    "xor": Code.XOR_RM32_IMM8,
    "add": Code.ADD_RM32_IMM8,
    "sub": Code.SUB_RM32_IMM8,
}
WIDE_IMM_EAX = {
    "and": Code.AND_EAX_IMM32,
    "or": Code.OR_EAX_IMM32,
    "xor": Code.XOR_EAX_IMM32,
    "add": Code.ADD_EAX_IMM32,
    "sub": Code.SUB_EAX_IMM32,
}
WIDE_IMM32 = {
    "and": Code.AND_RM32_IMM32,
    "or": Code.OR_RM32_IMM32,
    "xor": Code.XOR_RM32_IMM32,
    "add": Code.ADD_RM32_IMM32,
    "sub": Code.SUB_RM32_IMM32,
}


def relocated_memory(base: Register_) -> MemoryOperand:
    """A relocated address, always emitted as zero.

    LINK adds what is in the code to the fixup's target, so anything else
    would be added to the real address. An array element's fixup names the
    array's own base the same way, but the element it means also depends on
    whatever register indexes it -- carried through as this operand's own
    base, never optimised away to a bare displacement, which would silently
    mean a different element.
    """
    return MemoryOperand(base=base, displ=0, displ_size=2)


def memory(value: Value) -> MemoryOperand:
    """The operand as the widened instruction has to carry it."""
    match value.mem:
        case Addr(space=Space.SEGMENT, base=base):
            return relocated_memory(base)
        case Addr(space=Space.FRAME, disp=disp):
            return MemoryOperand(base=Register.BP, displ=disp, displ_size=value.dlen or 1)
        case Addr(space=Space.GROUP):
            # operand() already refuses this address kind, so no Value should
            # ever carry one here -- raising rather than falling into the
            # literal-displacement case below, which would silently emit a
            # group-relative offset as if it were a real one.
            raise ValueError(f"{value.op} has a group-relative operand -- operand() should have refused this")
        case Addr(disp=disp, base=base):
            return MemoryOperand(base=base, displ=disp, displ_size=2)
        case _:
            raise ValueError(f"{value.op} has no memory operand")


def instruction(value: Value) -> Instruction:
    """The one 386 instruction this value becomes."""
    wide = WIDE_REGISTER[value.pair]
    source = WIDE_REGISTER[value.src_pair]
    # pair 0 is eax, so a bare displacement takes the shorter moffs form --
    # but moffs has no ModRM byte at all, so it cannot carry an index register
    moffs = (
        value.pair == 0
        and value.mem is not None
        and value.mem.space is not Space.FRAME
        and value.mem.base == Register.NONE
    )
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
        case Op.ALUI if value.alu is not None and value.imm is not None and -128 <= value.imm < 128:
            return Instruction.create_reg_i32(WIDE_IMM8[value.alu], wide, value.imm)
        case Op.ALUI if value.alu is not None and value.imm is not None and value.pair == 0:
            return Instruction.create_reg_i32(WIDE_IMM_EAX[value.alu], wide, value.imm)
        case Op.ALUI if value.alu is not None and value.imm is not None:
            return Instruction.create_reg_i32(WIDE_IMM32[value.alu], wide, value.imm)
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
    if value.op is Op.CALL:
        if value.absorbed is None:
            raise ValueError("an Op.CALL value has no absorbed replacement to emit")
        return value.absorbed

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
    return any(values[i].op in (Op.ALUM, Op.ALUV, Op.ALUI, Op.NEG) for i in region if need[i])


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


def emit_region(
    values: list[Value], need: list[bool], region: list[int], live: Flag, dead: frozenset[int]
) -> Emitted | None:
    """The widened region, and the fixups it needs.

    `live` and `dead` are not optional and have no default: the gates these
    parameters carry were designed in and then lost once already (`live`, in a
    refactor -- see this function's own git history), and a default would make
    losing either again invisible.

    `dead` is the register pairs whose own dx/bx half is proven, by the
    caller's own cross-block liveness (qbopt.registers), never read before
    being overwritten again -- so restoring it would be pure waste, the
    residue.md I pattern. Filtered here rather than inside restored_pairs()
    itself: that function's own job is "which pairs does this region's value
    graph leave live", a fact about the region alone, and answering "is
    restoring one of them actually worth doing" needs the block graph this
    module has no notion of.

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

    restore = b"".join(FIXUP[pair] for pair in restored_pairs(values, need, region) if pair not in dead)
    return Emitted(bytes(out + restore), tuple(relocations))
