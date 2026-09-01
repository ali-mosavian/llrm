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
from qbopt.mir import Op
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
            for read in (_loads_from, _stores_to):
                made = _matched(first, second, read, body.origin)
                if made is not None:
                    out.append(made)
                    break
    return tuple(out)
