"""Strength reduction: a multiply in a loop becomes an add.

LLVM's `LoopStrengthReduce`, in its classic form. `induction.py` says which
values are affine recurrences; this rewrites the ones a loop recomputes.

    j = i * m        with i = {start,+,step} and m loop-invariant
    j   = {start*m,+,step*m}

So the multiply is not needed inside the loop at all. One multiply in the
preheader gives `start*m`; an add of `step*m` at the latch advances it; and
where BC wrote `imul word [w]` every iteration there is now an add.

**No phi is written here.** A fresh variable written in the preheader and
again at the latch *is* the phi -- `mir.resolved()` re-derives the SSA and
puts one at the header, because it renames per variable and that is what a
variable written on two paths means. Writing one by hand would be saying
the same thing twice, and the two would drift.

LLVM's LSR is far larger than this: it enumerates formulas for every use,
prices them against register pressure, and picks. That machinery exists
because a target with many addressing modes has many ways to write the same
address. BC has one, and it wrote the worst of them.
"""

from dataclasses import replace

from qbopt import mir
from qbopt import ssa
from qbopt.mir import Op
from qbopt import induction
from qbopt.mir import MirBody
from qbopt.passes import Where
from qbopt.passes import MIRTransform


class Strength(MIRTransform):
    name = "strength"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        return reduced(body, self.where.dgroup, self.where.bounds)


def reduced(body: MirBody, dgroup: frozenset[int] = frozenset(), bounds: dict | None = None) -> MirBody:
    """`body` with every multiply of a counter by an invariant made an add."""
    from qbopt import transform as passes

    found = induction.of(body, dgroup, bounds)
    if not found:
        return body

    at_of = {block.at: block for block in body.blocks}
    taken = max((one.variable for one in ssa.values(body)), default=0)
    first = taken + 1
    ahead: dict[int, list[Op]] = {}
    behind: dict[int, list[Op]] = {}
    swap: dict[int, mir.Value] = {}
    gone: set[int] = set()

    for loop, _basics, derived in found:
        preheader = passes._preheader(body, loop)
        latches = [at for at in loop.latches if at in at_of]
        if preheader is None or at_of[preheader].succ != (loop.header,) or len(latches) != 1:
            continue  # two ways in or out is a bigger change than this
        for one in derived:
            # Once each. A multiply inside a nest is derived in every loop
            # that contains it, and reducing it twice would set up two
            # counters for one value and delete the multiply once.
            answer = _answer(body, one.op)
            if answer is None or id(one.op) in gone or not _removable(at_of, one.op):
                continue
            width = _width(one.op)
            stride = _times(one.of.step, one.by, width)
            if stride is None:
                continue
            taken += 1
            start = mir.Value(id=_next(body, taken), at=preheader, variable=taken, version=1)
            step = mir.Value(id=start.id + 1, at=latches[0], variable=taken, version=2)

            ahead.setdefault(preheader, []).append(_start(start, one, preheader))
            behind.setdefault(latches[0], []).append(
                _made(
                    mir.Kind.ADD,
                    "add",
                    step,
                    (mir.Held(start, width), stride),
                    latches[0],
                    one.op,
                )
            )
            swap[answer.id] = start
            gone.add(id(one.op))

    if not gone:
        return body
    changed = replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(
                    _renamed(op, swap)
                    for op in _woven(block, ahead.get(block.at, []), behind.get(block.at, []), gone)
                ),
            )
            for block in body.blocks
        ),
    )
    return ssa.constructed(changed, frozenset(range(first, taken + 1)))


def _removable(at_of: dict, op: Op) -> bool:
    """Whether this multiply's own bytes can go to a survivor beside it.

    `_without` refuses a deletion no neighbour can account for, and leaves
    the operation standing -- which for this pass would compute the value
    twice, once in the loop and once in the counter its readers now name.
    Better to leave the multiply alone than to leave it *and* replace it.
    """
    from qbopt import mir as form
    from qbopt import transform as passes

    block = next((b for b in at_of.values() if any(one is op for one in b.ops)), None)
    if block is None:
        return False
    return len(passes._without(list(block.ops), lambda one: one is op)) < len(block.ops)


def _start(into, one, preheader: int) -> Op:
    """The counter's value on the way in: `start * by`, computed once.

    A multiply by one is a copy. It is what a word array gives -- `i shl 1`
    reduced against a step of one -- and emitting `imul r,1` is both longer
    and a form select does not have.
    """
    if isinstance(one.by, mir.Const) and one.by.n == 1:
        return _made(mir.Kind.COPY, "mov", into, (one.of.start,), preheader, one.op)
    return _made(mir.Kind.MUL, "imul", into, (one.of.start, one.by), preheader, one.op)


def _answer(body: MirBody, op: Op) -> "mir.Value | None":
    """The one value this multiply produces that anything reads, or None.

    A 16-bit `imul` writes dx:ax and the flags -- three values for one
    result. An add produces the low half and no more, so the reduction only
    applies where the low half is the whole of what the loop wanted. Where
    the high half or the flags are read too, the multiply is doing work an
    add does not do and it stays.
    """
    read = {
        value.id
        for block in body.blocks
        for other in block.ops
        if other is not op
        for value in other.uses
    }
    # A phi arm counts only where the phi's own result is read. The raise
    # makes a phi per register at every header, so dx appears in one after
    # every widening multiply whether or not anything wants its value --
    # and counting that as a read said the high half was wanted, which
    # refused every site in matrix.
    incoming = {phi.result.id: phi.incoming.values() for block in body.blocks for phi in block.phis}
    pending = list(read)
    while pending:
        for value in incoming.get(pending.pop(), ()):
            if value.id not in read:
                read.add(value.id)
                pending.append(value.id)
    wanted = [one for one in op.defines if one.id in read]
    if (
        len(wanted) != 1
        or wanted[0].flags
        or not op.results
        or not isinstance(op.results[0], mir.Held)
        or wanted[0] != op.results[0].value
    ):
        return None
    return wanted[0]


def _woven(block, ahead: list, behind: list, gone: set) -> list:
    """The block with the new counter set up and advanced, and the multiply out.

    `ahead` goes at the end of the preheader, after everything it may read.
    `behind` goes before whatever leaves the latch, because a branch reads
    the flags something before it set and a new add would be read as having
    changed them.
    """
    # The multiply's bytes go to a survivor in its own block. Every byte
    # between the first op and the last has to be accounted for, and an op
    # that simply disappears leaves a hole layout reports rather than emits
    # -- harr refused with "2 bytes between the ops are not instructions".
    from qbopt import transform as passes

    kept = passes._without(list(block.ops), lambda one: id(one) in gone)
    if ahead:
        cut = len(kept)
        while cut and kept[cut - 1].kind in (mir.Kind.JUMP, mir.Kind.BRANCH):
            cut -= 1
        kept = kept[:cut] + ahead + kept[cut:]
    if behind:
        cut = len(kept)
        while cut and kept[cut - 1].kind in (mir.Kind.JUMP, mir.Kind.BRANCH):
            cut -= 1
        kept = kept[:cut] + behind + kept[cut:]
    return kept


def _made(kind, name: str, into, args: tuple, at: int, beside: Op) -> Op:
    """One operation this pass invented, claiming none of BC's bytes."""
    return Op(
        at=beside.at,
        op=beside.op,
        name=name,
        defines=(into,),
        uses=tuple(one.value for one in args if isinstance(one, mir.Held)),
        loads=tuple(one.ref for one in args if isinstance(one, mir.Cell)),
        stores=(),
        node=None,
        made=None,
        covers=(beside.covers[0], beside.covers[0]) if beside.covers else None,
        kind=kind,
        args=args,
        results=(mir.Held(into, _widest(args)),),
    )


def _times(step, by, width: int):
    """`step * by`, where that can be said without an operation.

    A step of one is the case BC writes -- `FOR i = 1 TO n` -- and then the
    stride is the multiplier itself, whatever it is. Anything else needs a
    multiply of two invariants, which belongs in the preheader beside the
    first one and is not written yet.
    """
    if isinstance(step, mir.Const) and step.n == 1:
        return by
    if isinstance(step, mir.Const) and isinstance(by, mir.Const):
        return mir.Const(step.n * by.n, max(step.width, by.width))
    return None


def _renamed(op: Op, swap: dict) -> Op:
    """One operation reading the new counter where it read the multiply."""
    if not any(one.id in swap for one in op.uses) and not any(
        isinstance(one, mir.Held) and one.value.id in swap for one in op.args
    ):
        return op
    return replace(
        op,
        uses=tuple(swap.get(one.id, one) for one in op.uses),
        args=tuple(
            mir.Held(swap[one.value.id], one.width)
            if isinstance(one, mir.Held) and one.value.id in swap
            else one
            for one in op.args
        ),
    )


def _width(op: Op) -> int:
    for one in op.results:
        if isinstance(one, mir.Held):
            return one.width
    return 2


def _widest(args: tuple) -> int:
    return max((getattr(one, "width", 2) for one in args), default=2)


def _next(body: MirBody, taken: int) -> int:
    """An id nothing in this body uses."""
    seen = {0}
    for block in body.blocks:
        for op in block.ops:
            seen.update(one.id for one in (*op.defines, *op.uses))
        seen.update(phi.result.id for phi in block.phis)
    return max(seen) + 1 + taken * 2
