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
from dataclasses import dataclass

from iced_x86 import Register
from iced_x86 import Register_

from qbopt import ir
from qbopt import mir
from qbopt import wide
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
    ALU = "alu"


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
    what = op.made if op.made is not None else getattr(op.node, "semantics", None)
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


def _matched(first: Op, second: Op, read: object, origin: dict) -> Pair | None:
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
        for first, second in zip(ops, ops[1:]):
            made = _alu_adjacent(first, second, body.origin)
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
    what = op.made if op.made is not None else getattr(op.node, "semantics", None)
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
                case Kind.ALU if slots[pair.pair] is not None and low is not None and high is not None:
                    slots[pair.pair] = Long(low, high)
                case Kind.ALU:
                    slots[pair.pair] = None   # chaining from something unknown
                case Kind.STORE:
                    pass                      # reads the slot, leaves it alone
                case _:
                    slots[pair.pair] = None
    return out
