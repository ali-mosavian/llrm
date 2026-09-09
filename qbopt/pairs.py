"""
BC's two long register pairs, found over values instead of over bytes.

`lift.py` reads a long out of BC's 16-bit instruction stream by walking it
with two slots -- ax:dx and cx:bx -- and recognising about ten shapes, each
two instructions wide. This asks the same question of MIR, which is what M5
needs before widening can be a transform here rather than a byte rewrite.

The difference that matters is what joins the halves. `wide.pairs()` finds
the ones a *carry* joins -- `add ax,[x]` with `adc dx,[x+2]` -- and that is
169 of the 1,476 values lift finds across the corpus, 11 per cent. The rest
have no carry at all: a load pair is two independent `mov`s and the only
evidence they are one long is that the registers are a known pair and the
second address is two bytes above the first. Stores are the mirror. Between
them they are 59 per cent of the corpus's values and 72 per cent of
qb-qrender's, so they are where this has to start.

Nothing here widens anything. It answers "are these two ops the halves of
one 32-bit value", which is the question `transform.widened()` assumed away
and got wrong: it renamed `add ax,[x]`/`adc dx,[x+2]` to `add eax,[x]`, and
BC keeps that long in dx:ax, which is not eax. Knowing the pair is the step
before knowing which register it can become.
"""

from enum import StrEnum
from dataclasses import replace
from dataclasses import dataclass
from collections.abc import Callable

from iced_x86 import Register
from iced_x86 import Register_

from qbopt import ir
from qbopt import mir
from qbopt import wide
from qbopt import lower
from qbopt.mir import Op
from qbopt.mir import Value
from qbopt.mir import MirBody

# BC's own two, in lift.py's numbering and mir.RESTORE_PAIR's: pair 0 is
# ax:dx and pair 1 is cx:bx, low half first. A long lives in one of these
# and nowhere else -- which is what makes two slots enough rather than a
# general search over register pairs.
PAIRS: dict[int, tuple[Register_, Register_]] = {
    0: (Register.EAX, Register.EDX),
    1: (Register.ECX, Register.EBX),
}

HALF = 2


class Kind(StrEnum):
    LOAD = "load"
    STORE = "store"
    ALU = "alu"  # against memory
    ALU_IMM = "alu-i"  # against an immediate
    ALU_REG = "alu-v"  # against the other pair
    NOT = "not"
    MOVE = "move"  # pair to pair
    NEG = "neg"  # BC's three-instruction negate
    MOVSX = "movsx"  # an INTEGER sign-extended into a pair


@dataclass(frozen=True, slots=True)
class Pair:
    """Two ops that are the halves of one 32-bit value."""

    kind: Kind
    pair: int  # which of BC's two
    low: Op
    high: Op

    @property
    def at(self) -> tuple[int, int]:
        return (self.low.at, self.high.at)


def _moved(op: Op) -> ir.Semantics | None:
    """This op's semantics if it is a plain move, or None."""
    what = lower.current(op)
    if what is None or what.op is not ir.Operation.MOVE:
        return None
    return what if len(what.dests) == 1 and len(what.sources) == 1 else None


def _half_of(register: Register_, origin: dict) -> tuple[int, int] | None:
    """(which pair, which half) for a register, or None if it is in neither."""
    for number, (low, high) in PAIRS.items():
        if register is low:
            return number, 0
        if register is high:
            return number, 1
    return None


_Half = Callable[[Op], "tuple[Register_, object] | None"]


def _loads_from(op: Op) -> tuple[Register_, object] | None:
    """(destination root, cell) for `mov <half>,[x]`, or None."""
    what = _moved(op)
    if what is None or len(op.loads) != 1 or op.stores or op.loads[0].addr is None:
        return None
    dest = what.dests[0]
    if not isinstance(dest, ir.Reg) or dest.width != HALF:
        return None
    return ir.ROOT.get(dest.register, dest.register), op.loads[0]


def _stores_to(op: Op) -> tuple[Register_, object] | None:
    """(source root, cell) for `mov [x],<half>`, or None."""
    what = _moved(op)
    if what is None or len(op.stores) != 1 or op.loads or op.stores[0].addr is None:
        return None
    source = what.sources[0]
    if not isinstance(source, ir.Reg) or source.width != HALF:
        return None
    return ir.ROOT.get(source.register, source.register), op.stores[0]


def _adjacent(low: object, high: object) -> bool:
    """Whether `high` names the two bytes just above `low`.

    lift.py's own `+2`, and the whole evidence a load pair has: two moves
    with no carry between them are one long because the addresses say so.
    """
    a, b = getattr(low, "addr", None), getattr(high, "addr", None)
    if a is None or b is None:
        return False
    if getattr(low, "width", 0) != HALF or getattr(high, "width", 0) != HALF:
        return False
    return a.plus(HALF) == b


def _matched(first: Op, second: Op, read: _Half, origin: dict) -> Pair | None:
    """The two ops as one pair, if they are one, in either order.

    BC writes the halves low-first for a load and either way round for a
    store: `mov [bp-0Ch],dx` then `mov [bp-0Eh],ax` is one long, stored
    high half first. Requiring low-first missed 16 objects' worth, all of
    them that shape.
    """
    a, b = read(first), read(second)
    if a is None or b is None:
        return None
    kind = Kind.LOAD if read is _loads_from else Kind.STORE
    for (one, cell), (other, next_cell), low_op, high_op in (
        (a, b, first, second),
        (b, a, second, first),
    ):
        low_at, high_at = _half_of(one, origin), _half_of(other, origin)
        if low_at is None or high_at is None:
            continue
        if low_at[0] != high_at[0] or low_at[1] != 0 or high_at[1] != 1:
            continue
        if not _adjacent(cell, next_cell):
            continue
        return Pair(kind, low_at[0], low_op, high_op)
    return None


def found(body: MirBody) -> tuple[Pair, ...]:
    """Every adjacent load or store pair in `body`.

    Adjacent only, for now. lift.py allows an unrelated instruction between
    the halves -- docs/residue.md's E, address arithmetic for some other
    value -- and bridging it is a separate question from recognising the
    pair at all.
    """
    out: list[Pair] = []
    for block in body.blocks:
        ops = list(block.ops)
        for index, (first, second) in enumerate(zip(ops, ops[1:])):
            made = _negate(ops, index, body.origin)
            if made is not None:
                out.append(made)
                continue
            made = _sign_extended(first, second, body.origin)
            if made is not None:
                out.append(made)
                continue
            made = _alu_adjacent(first, second, body.origin)
            if made is None:
                made = _paired_alu(first, second, body.origin, ir.Imm, Kind.ALU_IMM)
            if made is None:
                made = _paired_alu(first, second, body.origin, ir.Reg, Kind.ALU_REG)
            if made is None:
                made = _unary_pair(first, second, body.origin)
            if made is None:
                made = _move_pair(first, second, body.origin)
            if made is not None:
                out.append(made)
                continue
            for read in (_loads_from, _stores_to):
                made = _matched(first, second, read, body.origin)
                if made is not None:
                    out.append(made)
                    break
    return tuple(out)


# What the high half's mnemonic must be, given the low half's. Only add and
# sub carry, which is the whole reason `wide.pairs()` cannot see the rest:
# it is keyed on the flags edge, and `and ax,[x]` / `and dx,[x+2]` has none.
# 169 of the corpus's alu pairs carry; lift.py finds 363.
PARTNER = {"and": "and", "or": "or", "xor": "xor", "add": "adc", "sub": "sbb"}


def _binary_on(op: Op) -> tuple[Register_, str, object] | None:
    """(destination root, mnemonic, cell) for `<alu> <half>,[x]`, or None."""
    what = lower.current(op)
    if what is None or what.op is not ir.Operation.BINARY:
        return None
    if len(what.dests) != 1 or len(op.loads) != 1 or op.stores or op.loads[0].addr is None:
        return None
    dest = what.dests[0]
    if not isinstance(dest, ir.Reg) or dest.width != HALF or dest not in what.sources:
        return None
    return ir.ROOT.get(dest.register, dest.register), (what.name or ""), op.loads[0]


def _alu_adjacent(first: Op, second: Op, origin: dict) -> "Pair | None":
    """An arithmetic pair recognised the way a load pair is.

    `and ax,[x]` with `and dx,[x+2]` is one 32-bit `and` and there is no
    carry between them to say so -- only the halves being a known pair, the
    addresses being two bytes apart, and the mnemonics being partners.
    """
    a, b = _binary_on(first), _binary_on(second)
    if a is None or b is None:
        return None
    (one, low_name, cell), (other, high_name, next_cell) = a, b
    low_at, high_at = _half_of(one, origin), _half_of(other, origin)
    if low_at is None or high_at is None:
        return None
    if low_at[0] != high_at[0] or low_at[1] != 0 or high_at[1] != 1:
        return None
    if PARTNER.get(low_name) != high_name or not _adjacent(cell, next_cell):
        return None
    return Pair(Kind.ALU, low_at[0], first, second)


def _halves_named(first: Op, second: Op, origin: dict) -> tuple[int, Op, Op] | None:
    """(pair number, low op, high op) where these two write one pair's halves.

    Each op must write exactly one tracked register and between them they
    must be the low and the high of the same pair -- in either order, since
    BC writes a store's halves either way round.
    """
    for one, other, low_op, high_op in ((first, second, first, second), (second, first, second, first)):
        low = _half_of(_written(one, origin), origin)
        high = _half_of(_written(other, origin), origin)
        if low is None or high is None:
            continue
        if low[0] == high[0] and low[1] == 0 and high[1] == 1:
            return low[0], low_op, high_op
    return None


def _written(op: Op, origin: dict) -> Register_:
    """The one tracked register this op writes, or NONE."""
    made = [one for one in op.defines if not one.flags]
    if len(made) != 1:
        return Register.NONE
    return origin.get(made[0], Register.NONE)


def _shape(op: Op) -> ir.Semantics | None:
    return lower.current(op)


def _binary_against(op: Op, want: type) -> str | None:
    """This op's mnemonic if it is `<alu> <half>,<want>` in place, else None."""
    what = _shape(op)
    if what is None or what.op is not ir.Operation.BINARY or len(what.dests) != 1:
        return None
    dest = what.dests[0]
    if not isinstance(dest, ir.Reg) or dest.width != HALF or dest not in what.sources:
        return None
    if op.loads or op.stores:
        return None
    if not any(isinstance(one, want) for one in what.sources):
        return None
    return what.name or ""


def _paired_alu(first: Op, second: Op, origin: dict, want: type, kind: Kind) -> "Pair | None":
    """An arithmetic pair whose operand is not memory.

    There is no second address to compare here, so the evidence is the
    register pair and the mnemonics being partners -- which is what lift.py
    accepts for the same shapes, and it chains from a known value rather
    than establishing one.
    """
    named = _halves_named(first, second, origin)
    if named is None:
        return None
    number, low_op, high_op = named
    low_name, high_name = _binary_against(low_op, want), _binary_against(high_op, want)
    if low_name is None or high_name is None or PARTNER.get(low_name) != high_name:
        return None
    return Pair(kind, number, low_op, high_op)


def _unary_pair(first: Op, second: Op, origin: dict) -> "Pair | None":
    """`not ax` with `not dx` -- one 32-bit not, and BC's own shape for it."""
    named = _halves_named(first, second, origin)
    if named is None:
        return None
    number, low_op, high_op = named
    names = []
    for op in (low_op, high_op):
        what = _shape(op)
        if what is None or what.op is not ir.Operation.UNARY or op.loads or op.stores:
            return None
        names.append(what.name or "")
    if names[0] != "not" or names[1] != "not":
        return None
    return Pair(Kind.NOT, number, low_op, high_op)


def _move_pair(first: Op, second: Op, origin: dict) -> "Pair | None":
    """`mov ax,cx` with `mov dx,bx` -- one pair copied into the other.

    A copy is a value, not nothing: the source pair is usually reused
    immediately afterwards, so the copy is what keeps the long alive.
    """
    named = _halves_named(first, second, origin)
    if named is None:
        return None
    number, low_op, high_op = named
    sources = []
    for op in (low_op, high_op):
        what = _shape(op)
        if what is None or what.op is not ir.Operation.MOVE or op.loads or op.stores:
            return None
        if len(what.sources) != 1 or not isinstance(what.sources[0], ir.Reg):
            return None
        if what.sources[0].width != HALF:
            return None
        sources.append(_half_of(ir.ROOT.get(what.sources[0].register, what.sources[0].register), origin))
    if sources[0] is None or sources[1] is None:
        return None
    if sources[0][0] != sources[1][0] or sources[0][1] != 0 or sources[1][1] != 1:
        return None
    if sources[0][0] == number:
        return None  # a pair copied onto itself is not a move between pairs
    return Pair(Kind.MOVE, number, low_op, high_op)


def _negate(ops: list[Op], index: int, origin: dict) -> "Pair | None":
    """`neg ax / adc dx,0 / neg dx` -- BC's own three-instruction long negate.

    Three instructions rather than two halves side by side, so it does not
    fit the pair-of-ops shape the rest of this module uses. The middle one
    is what makes it a negate and not two independent ones: `adc dx,0` folds
    the borrow the low half's `neg` produced into the high half before it is
    negated in turn.
    """
    if index + 2 >= len(ops):
        return None
    first, middle, last = ops[index], ops[index + 1], ops[index + 2]
    low = _half_of(_written(first, origin), origin)
    high = _half_of(_written(last, origin), origin)
    if low is None or high is None or low[0] != high[0] or low[1] != 0 or high[1] != 1:
        return None
    shapes = [_shape(one) for one in (first, middle, last)]
    if any(one is None for one in shapes):
        return None
    shapes = [one for one in shapes if one is not None]
    if shapes[0].op is not ir.Operation.UNARY or (shapes[0].name or "") != "neg":
        return None
    if shapes[2].op is not ir.Operation.UNARY or (shapes[2].name or "") != "neg":
        return None
    if shapes[1].op is not ir.Operation.BINARY or (shapes[1].name or "") != "adc":
        return None
    if _half_of(_written(middle, origin), origin) != (low[0], 1):
        return None
    return Pair(Kind.NEG, low[0], first, last)


def _sign_extended(first: Op, second: Op, origin: dict) -> "Pair | None":
    """`mov ax,<source>` then `cwd` -- an INTEGER widened into pair 0.

    `cwd` is not an operation on a pair; it is what turns one half into
    both. lift.py could not see this at all before it was added there: cwd
    is not a value it tracks, so the sequence fell through to unrecognised
    and cleared everything. 90 of qb-qrender's 522 values are this shape,
    which is what an integer-heavy program looks like.
    """
    low = _half_of(_written(first, origin), origin)
    high = _half_of(_written(second, origin), origin)
    if low is None or high is None or low != (0, 0) or high != (0, 1):
        return None
    made = _shape(first)
    if made is None or made.op is not ir.Operation.MOVE or len(made.sources) != 1:
        return None
    if isinstance(made.sources[0], ir.Imm):
        return None  # calls.widened_constant_at()'s own, narrower shape
    if isinstance(made.sources[0], ir.Reg) and made.sources[0].register in mir.PHYSICAL:
        # `mov ax,es / cwd` is the shape and not the meaning: a segment
        # register is not a value here (mir.PHYSICAL says so), so the long
        # this would seed has a half nothing can account for.
        return None
    widening = _shape(second)
    if widening is None or (widening.name or "") != "cwd":
        return None
    return Pair(Kind.MOVSX, 0, first, second)


def _alu(body: MirBody) -> list[Pair]:
    """The carry-joined pairs, which are `wide.pairs()`'s own question.

    An `add ax,[x]` with `adc dx,[x+2]` is joined by the flags value the
    first defines and the second reads, and that edge is the evidence --
    there is no second address to compare where the operand is a register or
    an immediate. Reusing wide.py rather than re-deriving it: the carry half
    of this question was already answered.
    """
    out: list[Pair] = []
    for one in wide.pairs(body):
        low = _half_of(_root_of(one.low, body), body.origin)
        high = _half_of(_root_of(one.high, body), body.origin)
        if low is None or high is None:
            continue
        if low[0] != high[0] or low[1] != 0 or high[1] != 1:
            continue
        out.append(Pair(Kind.ALU, low[0], one.low, one.high))
    return out


def _root_of(op: Op, body: MirBody) -> Register_:
    """The 32-bit register this op's own result lands in, or NONE."""
    made = [one for one in op.defines if not one.flags]
    if len(made) != 1:
        return Register.NONE
    return body.origin.get(made[0], Register.NONE)


@dataclass(frozen=True, slots=True)
class Long:
    """A 32-bit value living in one of BC's pairs, as its two halves."""

    low: Value
    high: Value


def _halves(op: Op, body: MirBody) -> Value | None:
    made = [one for one in op.defines if not one.flags]
    return made[0] if len(made) == 1 else None


def _touches(op: Op, number: int, body: MirBody) -> bool:
    """Whether this op writes either half of pair `number`."""
    low, high = PAIRS[number]
    return any(body.origin.get(one) in (low, high) for one in op.defines if not one.flags)


def held(body: MirBody) -> dict[int, dict[int, Long | None]]:
    """What each of BC's two pairs holds just before every op.

    lift.py's `live` dict, over values. A load puts a long in a slot, an
    arithmetic pair chains from what is there, a store reads it, and
    **anything else that writes either half clears it** -- which is the rule
    that makes the whole thing sound and the one a shape-only recogniser
    does not have. `pairs.found()` says two ops are one 32-bit access;
    this says what is in the pair when they run.

    That difference is not academic. After a runtime call returning a long
    in dx:ax, `mov ds:[0],ax` / `mov ds:[0],dx` is a real 32-bit store whose
    value this cannot name, because the call cleared the slot. Widening it
    needs the call's own contract, not the shape.
    """
    every = {min(one.at): one for one in (*found(body), *_alu(body))}
    out: dict[int, dict[int, Long | None]] = {}

    for block in body.blocks:
        slots: dict[int, Long | None] = {0: None, 1: None}
        skip: set[int] = set()
        for op in block.ops:
            out[op.at] = dict(slots)
            if op.at in skip:
                continue
            pair = every.get(op.at)
            if pair is None or (pair.low.at != op.at and pair.high.at != op.at):
                if op.barrier or op.at in getattr(body, "calls", ()):
                    slots = {0: None, 1: None}
                    continue
                for number in PAIRS:
                    if _touches(op, number, body):
                        slots[number] = None
                continue

            skip.add(pair.high.at if pair.low.at == op.at else pair.low.at)
            low, high = _halves(pair.low, body), _halves(pair.high, body)
            match pair.kind:
                case Kind.LOAD if low is not None and high is not None:
                    slots[pair.pair] = Long(low, high)
                case Kind.ALU | Kind.ALU_IMM | Kind.ALU_REG | Kind.NOT if (
                    slots[pair.pair] is not None and low is not None and high is not None
                ):
                    slots[pair.pair] = Long(low, high)
                case Kind.ALU | Kind.ALU_IMM | Kind.ALU_REG | Kind.NOT:
                    slots[pair.pair] = None  # chaining from something unknown
                case Kind.NEG if slots[pair.pair] is not None and low is not None and high is not None:
                    slots[pair.pair] = Long(low, high)
                case Kind.NEG:
                    slots[pair.pair] = None
                case Kind.MOVSX if low is not None and high is not None:
                    # a sign extension seeds the pair the way a load does:
                    # it establishes a long rather than chaining from one
                    slots[pair.pair] = Long(low, high)
                case Kind.MOVE if low is not None and high is not None:
                    # the destination takes what the source pair holds, and
                    # a copy from an unknown pair leaves the destination
                    # unknown too
                    slots[pair.pair] = Long(low, high)
                case Kind.STORE:
                    pass  # reads the slot, leaves it alone
                case _:
                    slots[pair.pair] = None
    return out


# What each pair becomes when the whole chain is widened. lift.py's own map:
# a long in ax:dx becomes eax, one in cx:bx becomes ecx.
WIDE = {0: Register.EAX, 1: Register.ECX}

# `push eax / pop ax / pop dx` and its cx:bx twin -- what hands the long back
# to BC's sixteen-bit code at the end of a chain. Four bytes, and skippable
# only where the pair is provably dead afterwards.
RESTORE = 4


@dataclass(frozen=True, slots=True)
class Chain:
    """A run of pair operations on one slot, and what widening it would cost."""

    pair: int
    ops: tuple[Pair, ...]
    was: int  # bytes BC wrote
    now: int  # bytes the widened form needs, restore included
    restored: bool

    @property
    def saved(self) -> int:
        return self.was - self.now

    @property
    def at(self) -> int:
        return min(min(one.at) for one in self.ops)


def _span(one: Pair) -> int:
    """The bytes BC wrote for both halves of this pair operation."""
    total = 0
    for op in (one.low, one.high):
        if op.node is None:
            return 0
        lo, hi = ir.span(op.node)
        total += hi - lo
    return total


def _semantics_of(one: Op) -> ir.Semantics | None:
    return lower.current(one)


def _widened_length(one: Pair) -> int | None:
    """How many bytes the one 32-bit instruction takes, or None if unknown."""
    from qbopt import select

    wide = wider(one)
    if wide is None:
        return None
    built = select.emit(wide, at=0)
    return None if built is None else len(built.code)


def wider(pair: Pair) -> ir.Semantics | None:
    """A pair of 16-bit operations as the one 32-bit operation they are.

    Each register becomes its own root and each memory operand doubles its
    width -- which is only right for a chain that ends in a restore, and is
    exactly the assumption that made an isolated rename wrong.

    An immediate is the part that is not a rename. BC splits a long constant
    across the two instructions, so `and ax,0ffffh / and dx,7fffh` is one
    `and eax,7fffffffh` and the low half alone says `0ffffh` -- which as a
    32-bit immediate is a different constant, and one that clears the high
    half of every value it is applied to. 224 of the corpus's pairs carry a
    high half that is not zero.
    """
    from dataclasses import replace as _replace

    what = _semantics_of(pair.low)
    if what is None:
        return None

    high = _semantics_of(pair.high)
    top = next((one.value for one in high.sources if isinstance(one, ir.Imm)), None) if high else None

    def wider_loc(where: ir.Loc) -> ir.Loc | None:
        match where:
            case ir.Reg(register=register):
                return ir.Reg(register=ir.ROOT.get(register, register), width=4)
            case ir.Mem():
                return _replace(where, width=4)
            case ir.Imm(value=value):
                if top is None:
                    return None
                whole = ((top & 0xFFFF) << 16) | (value & 0xFFFF)
                return ir.Imm(value=whole - (1 << 32) if whole & 0x80000000 else whole, width=4)
            case _:
                return where

    dests = [wider_loc(one) for one in what.dests]
    sources = [wider_loc(one) for one in what.sources]
    if any(one is None for one in (*dests, *sources)):
        return None  # an immediate whose high half is not an immediate
    # `xor cx,ax / xor bx,dx` is one `xor ecx,eax`, and that is only the
    # same operation if eax already holds the whole long -- which is true
    # only where the *other* pair was widened over the same stretch, and
    # nothing here knows that. Refused rather than assumed.
    into = dests[0] if dests and isinstance(dests[0], ir.Reg) else None
    if into is not None and any(isinstance(one, ir.Reg) and one.register != into.register for one in sources):
        return None
    return _replace(what, dests=tuple(dests), sources=tuple(sources))


def _ends(one: Pair) -> int | None:
    ends = [
        (op.covers if op.covers is not None else ir.span(op.node))[1]
        for op in (one.low, one.high)
        if op.covers is not None or op.node is not None
    ]
    return max(ends) if ends else None


def _follows(previous: Pair, one: Pair, block=None, origin: dict | None = None) -> bool:
    """Whether `one` continues `previous`, with nothing in between that stops it.

    Contiguity was lift.regions()' rule and this kept it: anything
    unrecognised between two pair operations has to stay where it is, so a
    rewrite cannot span it. docs/residue.md's E is that rule costing real
    chains -- BC drops address arithmetic for some *other* value between the
    halves of one long expression.

    MIR can ask the question the machine arm could not. What stops a chain
    is an instruction that touches the pair's own registers, and `defines`
    and `uses` say exactly which values an op reads and writes; anything
    else in the gap is unrelated and is carried through where it stood.

    A barrier still stops it: what it touches is its encoding's business.
    """
    ends = _ends(previous)
    if ends is None:
        return False
    lo = min(one.at)
    if ends == lo:
        return True
    if block is None or origin is None:
        return False
    roots = PAIRS[previous.pair]
    for op in block.ops:
        if not (ends <= op.at < lo):
            continue
        if op.barrier or op.at in getattr(block, "calls", ()):
            return False
        if {origin.get(value) for value in (*op.defines, *op.uses)} & set(roots):
            return False
    return True


def chains(body: MirBody, dead: frozenset[int] = frozenset()) -> tuple[Chain, ...]:
    """Every widenable run, with what widening it would cost.

    A chain is a maximal run of pair operations on one slot whose halves are
    contiguous in the code -- anything unrecognised between them has to stay
    where it is, so the rewrite cannot span it, which is lift.regions()'s
    own rule.

    `dead` is the pairs provably dead after the body, whose restore can be
    skipped. Without it every chain pays the four bytes.

    The cost is the whole point. A single pair widened is usually *longer*
    than the two instructions BC wrote once the restore is counted, which is
    why lift.py refuses 156 regions in qb-qrender against the ones it takes.
    """
    state = held(body)
    every = {min(one.at): one for one in found(body)}
    out: list[Chain] = []

    for block in body.blocks:
        runs: dict[int, list[Pair]] = {0: [], 1: []}

        def close(number: int) -> None:
            run = runs[number]
            if not run:
                return
            was = sum(_span(one) for one in run)
            widths = [_widened_length(one) for one in run]
            runs[number] = []
            if was == 0 or any(one is None for one in widths):
                return
            restored = number not in dead
            now = sum(one for one in widths if one is not None) + (RESTORE if restored else 0)
            out.append(Chain(number, tuple(run), was, now, restored))

        for op in block.ops:
            one = every.get(op.at)
            if one is None:
                continue
            number = one.pair
            # a run breaks where the slot stops being known, and where the
            # previous member is not immediately before this one
            known = state.get(min(one.at), {}).get(number) is not None
            if runs[number] and not _follows(runs[number][-1], one, block, body.origin):
                close(number)
            if one.kind is Kind.LOAD:
                close(number)
                runs[number] = [one]
                continue
            if one.kind is Kind.MOVSX:
                # `mov ax,[x] / cwd` is a sign extension, and widening it as
                # the low half's own semantics reads four bytes out of a
                # two-byte cell. The right instruction is `movsx eax,[x]`
                # and select.py has no form for it, so a chain neither
                # starts on one nor spans one.
                close(number)
                continue
            if not known or not runs[number]:
                # Nothing widened put this pair in the 32-bit register, so
                # its high half holds whatever BC last left in dx -- and a
                # chain that starts on arithmetic operates on that. The slot
                # being known says the value is tracked, not that it is in
                # one register: 13 of the corpus's chains began this way.
                close(number)
                continue
            runs[number].append(one)
        for number in (0, 1):
            close(number)
    return tuple(out)


def _widened_arg(one):
    """One operand of a pair's low half, at the width the whole pair uses.

    The two halves are one four-byte operation, so its operands are four
    bytes -- and an operand still saying two is what made the widened form
    read as a half of something.
    """
    if isinstance(one, mir.Held) and one.width == HALF:
        return mir.Held(one.value, HALF * 2)
    if isinstance(one, mir.Cell) and one.ref.width == HALF:
        return mir.Cell(replace(one.ref, width=HALF * 2))
    return one


def _handed(chain: Chain) -> tuple:
    """The values the restore hands back, from the chain's last pair.

    Only the halves something after the chain still reads: the whole value
    the widened operation defines is the low half's own id, so defining it
    again would say one value is written twice.
    """
    last = chain.ops[-1]
    whole = {one for one in last.low.defines if not one.flags}
    return tuple(one for one in last.high.defines if not one.flags and one not in whole)


def _restore_op(number: int, at: int, after: Op, end: int, hands: tuple = ()) -> Op:
    """`push eax / pop ax / pop dx` -- the long handed back to BC's halves.

    One op carrying an ir.Restore, not three ordinary ones. Three would each
    need an address of their own, and layout.rebuild() orders every op by
    address, so they would have to be threaded between the addresses the
    chain is dropping -- which a two-operation chain does not have enough
    of. One node needs one address, and the last half this chain drops is
    always past its last low, so it lands where it belongs.

    `covers` runs from there to the end of the chain, which is the rest of
    BC's bytes once the widened operations have claimed theirs. It is not
    four, and does not have to be: what this emits is four bytes, and
    layout.py asks select.restore() rather than covers for that.
    """
    node = ir.Restore(at=at, end=at + RESTORE, pair=number, effects=ir.RESTORE_EFFECTS[number])
    # A barrier, and defining and using nothing. It used to be built by
    # replacing the op before it, which handed it that op's own values: the
    # restore claimed to define what the widened operation defined, and
    # avail.py reasons on exactly that. What it really does -- write both of
    # BC's sixteen-bit halves out of the wide register -- is not something
    # this can name in SSA from here, so it says nothing and stops anything
    # reasoning across it instead.
    return replace(
        after,
        at=at,
        op=ir.Operation.BARRIER,
        kind=mir.Kind.OPAQUE,
        name="restore",
        # What it hands back. It used to define nothing so that avail.py
        # could not reason across it -- the barrier below still stops that
        # -- but a half nothing defines is a half the allocator never
        # places: negnot pushed whatever dx held and printed A=-7317895
        # for 305419897.
        defines=hands,
        uses=(),
        loads=(),
        stores=(),
        # And no operands. Built by replacing the operation before it, so
        # without these it inherited that one's -- and a lowering that
        # builds semantics for an operation nothing rewrote then gave the
        # idiom three operands it never had. select emitted nothing for
        # it, the widened pair was never split back, and arith pushed a
        # stale high word: AND= 1544 for 33818120.
        args=(),
        results=(),
        node=node,
        made=None,
        covers=(at, end),
    )


# The sixteen-bit name of each root, which is what a restore's pops name.
_NARROW_OF = {
    Register.EAX: Register.AX,
    Register.EDX: Register.DX,
    Register.ECX: Register.CX,
    Register.EBX: Register.BX,
}


def replaced(chain: Chain, block) -> tuple[int, int, set[int]]:
    """A chain's span, and every instruction inside it the widening replaces.

    Everything in the span, not only the halves each Pair names. A negate is
    three instructions -- `neg ax / adc dx,0 / neg dx` -- and the middle one
    belongs to no Pair, so dropping by member left the `adc` standing while
    the widened `neg eax` claimed its bytes: a stale carry into a high half
    that no longer had one, and two ops on a single address. The chain is
    contiguous by construction -- _follows() is what builds it -- so
    everything inside it is part of it.
    """
    lo = min(min(pair.at) for pair in chain.ops)
    hi = max(end for pair in chain.ops if (end := _ends(pair)) is not None)
    # Each pair's own stretch, not the whole span. A chain may now step over
    # an instruction that touches neither of its registers -- residue.md's E
    # -- and that instruction stays exactly where BC put it, so it is not
    # this chain's to remove.
    mine: set[int] = set()
    for pair in chain.ops:
        start, end = min(pair.at), _ends(pair)
        if end is None:
            continue
        mine.update(one.at for one in block.ops if start <= one.at < end)
    return lo, hi, mine


def widened(body: MirBody, dead: frozenset[int] = frozenset()) -> MirBody:
    """Every chain worth widening, as 32-bit operations on one register.

    A chain of N pair operations becomes N single instructions plus the
    restore, and only where that is *shorter* -- the cost model is not a
    refinement here, it is what stops the transform making qb-qrender
    bigger. 235 of its 341 chains would grow.

    The first widened operation carries the whole chain's original bytes in
    `covers`, and everything after it carries none, so layout.py's own
    accounting still adds up. Addresses for the new operations come from the
    halves being dropped, so nothing collides.
    """
    taken = [one for one in chains(body, dead) if one.saved > 0]
    if not taken:
        return body

    starts = {one.at: one for one in taken}
    inside = {min(pair.at) for one in taken for pair in one.ops}
    blocks = []
    pins: dict = {}
    for block in body.blocks:
        ops: list[Op] = []
        drop: set[int] = set()
        for op in block.ops:
            if op.at in drop:
                continue
            chain = starts.get(op.at)
            if chain is None:
                if op.at in inside:
                    continue  # a half whose chain already emitted it
                ops.append(op)
                continue

            lo, hi, mine = replaced(chain, block)
            drop.update(mine)
            # The restore goes on the chain's last high half: the one
            # address in the span that is past every low and belongs to no
            # widened operation. It used to go four bytes back from the end,
            # to make its `covers` exactly the four bytes select.restore()
            # emits -- but a chain ending in a two-and-two pair has its last
            # low on that very address, and the two ops collided. What an op
            # emits and which of BC's bytes it stands for are separate
            # questions, and layout.py now measures the restore by the first.
            #
            # It is not always past every low: BC writes a long store high
            # half first, so a chain ending in one puts the restore an
            # instruction before its own widened store. That is safe for the
            # reason the idiom is `push eax / pop ax / pop dx` -- eax comes
            # out of it holding what it held -- and only for an op that
            # reads the wide register rather than writing it. A store is the
            # only kind that can land there, and
            # test_only_a_store_may_follow_the_restore is what keeps it so.
            _restore_at = chain.ops[-1].high.at if chain.restored else hi
            last = len(chain.ops) - 1
            for number, pair in enumerate(chain.ops):
                what = wider(pair)
                assert what is not None
                # Each widened operation stands for its own pair's bytes and
                # no more, so an instruction carried through a gap still owns
                # the bytes it sits on and layout.py's arithmetic adds up.
                start, end = min(pair.at), _ends(pair)
                # MIR says it too, not only `made`: the widened operation is
                # four bytes wide, and operands still saying two made the one
                # 32-bit subtract read as a half of something. The kind is
                # the low half's own and is already right -- the low half of
                # a subtract is a subtract.
                ops.append(
                    replace(
                        pair.low,
                        args=tuple(_widened_arg(one) for one in pair.low.args),
                        results=tuple(_widened_arg(one) for one in pair.low.results),
                        made=what,
                        covers=(start, _restore_at if number == last else end),
                    )
                )
            if chain.restored:
                ops.append(_restore_op(chain.pair, _restore_at, ops[-1], hi, _handed(chain)))
                pins.update(_handed_back(chain, body.origin))
            drop.discard(op.at)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks), pins={**body.pins, **pins})


def _handed_back(chain: Chain, origin: dict) -> dict:
    """The two halves the restore writes, each held to the register BC read.

    `pop ax / pop dx` names its registers in its own encoding, and the
    restore defines nothing in SSA so that avail cannot reason across it.
    The allocator reads the same field: it found the values BC's last pair
    defined with no definition left and moved them, and `push dx` came out
    `push cx`. negnot printed B= 1545 for 33818121.
    """
    last = chain.ops[-1]
    return {
        one: origin[one] for half in (last.low, last.high) for one in half.defines if not one.flags and one in origin
    }
