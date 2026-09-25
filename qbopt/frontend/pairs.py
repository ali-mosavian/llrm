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
one 32-bit value" for the raise-side scalar recognizer. Which register the
whole eventually occupies belongs to allocation.
"""

from enum import StrEnum
from dataclasses import dataclass
from collections.abc import Callable

from iced_x86 import Register
from iced_x86 import Register_

from qbopt.model import ir
from qbopt.model import mir
from qbopt.model.mir import Op
from qbopt.backend import lower
from qbopt.model.mir import MirBody

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

    @property
    def first(self) -> Op:
        return min((self.low, self.high), key=lambda op: op.at)


def _moved(op: Op) -> ir.Semantics | None:
    """This op's semantics if it is a plain move, or None."""
    what = _shape(op)
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
    the halves -- docs/optimizations/residue.md's E, address arithmetic for some other
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
    what = _shape(op)
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
    if op.floating_origin is not None:
        return None
    return lower.current(op, node=getattr(op, "node", None))


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
