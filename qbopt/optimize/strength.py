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
address. Both inner and outer loops are eligible. Allocation owns pressure
and spilling: an older blanket ban on outer recurrences outlived the
allocator behavior that motivated it and retained NESTED's row multiplies.
"""

from dataclasses import replace

from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.model.mir import Op
from qbopt.analysis import induction
from qbopt.model.mir import MirBody
from qbopt.model.passes import Where
from qbopt.model.passes import MIRTransform


class Strength(MIRTransform):
    name = "strength"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        from qbopt.optimize import floatloop, loopexit

        body = loopexit.evaluated(reduced(body, self.where.dgroup, self.where.bounds))
        return floatloop.specialized(body, self.where.dgroup, self.where.calls)


def reduced(body: MirBody, dgroup: frozenset[int] = frozenset(), bounds: dict | None = None) -> MirBody:
    """`body` with every multiply of a counter by an invariant made an add."""
    from qbopt.optimize import transform as passes

    found = induction.of(body, dgroup, bounds)
    if not found:
        return body

    at_of = {block.at: block for block in body.blocks}
    taken = max((one.variable for one in ssa.values(body)), default=0)
    first = taken + 1
    ahead: dict[int, list[Op]] = {}
    behind: dict[int, list[Op]] = {}
    replacements: dict[int, Op] = {}
    for loop, _basics, derived in found:
        preheader = passes._preheader(body, loop)
        latches = [at for at in loop.latches if at in at_of]
        if preheader is None or at_of[preheader].succ != (loop.header,) or len(latches) != 1:
            continue  # two ways in or out is a bigger change than this
        candidates = [
            one
            for one in derived
            if _answer(body, one.op) is not None
            and (
                _multiplies(one, derived)
                or one.op.kind is mir.Kind.DIVMOD
                or one.offsets
                and one.op.kind is mir.Kind.SHL
                or any(isinstance(offset, mir.Cell) for offset, _ in one.offsets)
                or one.op.kind is mir.Kind.ADD
                and any(isinstance(offset, mir.Held) for offset, _ in one.offsets)
            )
        ]
        consumed = {arg.value for one in candidates for arg in one.op.args if isinstance(arg, mir.Held)}
        candidates = [one for one in candidates if one.op.results[0].value not in consumed]
        for one in candidates:
            # Once each. A multiply inside a nest is derived in every loop
            # that contains it, and reducing it twice would set up two
            # counters for one value.
            answer = _answer(body, one.op)
            if answer is None or id(one.op) in replacements:
                continue
            width = _width(one.op)
            if _times(one.of.step, one.by, width) is None:
                continue
            if isinstance(one.by, mir.Cell):
                if one.op.loads != (one.by.ref,) or one.by not in one.op.args:
                    continue
                taken += 1
                multiplier = mir.Value(id=_next(body, taken), at=preheader, variable=taken, version=1)
                load = _made(mir.Kind.LOAD, "mov", multiplier, (one.by,), preheader, one.op)
                ahead.setdefault(preheader, []).append(replace(load, id=one.op.id, symbol=True))
                one = replace(one, by=mir.Held(multiplier, width))
            stride = _times(one.of.step, one.by, width)
            if stride is None:
                continue
            taken += 1
            start = mir.Value(id=_next(body, taken), at=preheader, variable=taken, version=1)
            step = mir.Value(id=start.id + 1, at=latches[0], variable=taken, version=2)

            ahead.setdefault(preheader, []).extend(_starts(start, one, preheader))
            taken += len(one.offsets) * 2
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
            replacements[id(one.op)] = replace(
                one.op,
                kind=mir.Kind.COPY,
                name="",
                node=None,
                made=None,
                defines=(answer,),
                uses=(start,),
                args=(mir.Held(start, width),),
                results=(mir.Held(answer, width),),
                loads=(),
                stores=(),
                merges={},
                symbol=False,
            )

    if not replacements:
        return body
    changed = replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(_woven(block, ahead.get(block.at, []), behind.get(block.at, []), replacements)),
            )
            for block in body.blocks
        ),
    )
    return ssa.constructed(changed, frozenset(range(first, taken + 1)))


def _multiplies(one: induction.Derived, derived: list[induction.Derived]) -> bool:
    """Replace multiplication chains, not cheap shifts needing extra counters."""
    producers = {
        item.op.results[0].value: item.op
        for item in derived
        if item.of == one.of and item.op.results and isinstance(item.op.results[0], mir.Held)
    }
    pending = [one.op]
    seen = set()
    while pending:
        op = pending.pop()
        if id(op) in seen:
            continue
        seen.add(id(op))
        if op.kind is mir.Kind.MUL:
            return True
        pending.extend(producers[arg.value] for arg in op.args if isinstance(arg, mir.Held) and arg.value in producers)
    return False


def _start(into, one, preheader: int) -> Op:
    """The counter's value on the way in: `start * by`, computed once.

    A multiply by one is a copy. It is what a word array gives -- `i shl 1`
    reduced against a step of one -- and emitting `imul r,1` is both longer
    and a form select does not have.
    """
    if isinstance(one.by, mir.Const) and one.by.n == 1:
        return _made(mir.Kind.COPY, "mov", into, (one.of.start,), preheader, one.op)
    return _made(mir.Kind.MUL, "imul", into, (one.of.start, one.by), preheader, one.op)


def _starts(into: mir.Value, one: induction.Derived, preheader: int) -> list[Op]:
    """Initialize scale * start plus invariant offsets once, before the loop."""
    if not one.offsets:
        return [_start(into, one, preheader)]
    width = _width(one.op)
    temporaries = iter(
        mir.Value(into.id + 2 + number, preheader, variable=into.variable + 1 + number, version=1)
        for number in range(len(one.offsets) * 2)
    )
    current = next(temporaries)
    operations = [_start(current, one, preheader)]
    for index, (offset, coefficient) in enumerate(one.offsets):
        if coefficient != 1:
            product = next(temporaries)
            operations.append(
                _made(
                    mir.Kind.MUL,
                    "imul",
                    product,
                    (offset, mir.Const(coefficient & 0xFFFF, width)),
                    preheader,
                    one.op,
                )
            )
            offset = mir.Held(product, width)
        result = into if index == len(one.offsets) - 1 else next(temporaries)
        operations.append(_made(mir.Kind.ADD, "add", result, (mir.Held(current, width), offset), preheader, one.op))
        current = result
    return operations


def _answer(body: MirBody, op: Op) -> "mir.Value | None":
    """The one value this multiply produces that anything reads, or None.

    A 16-bit `imul` writes dx:ax and the flags -- three values for one
    result. An add produces the low half and no more, so the reduction only
    applies where the low half is the whole of what the loop wanted. Where
    the high half or the flags are read too, the multiply is doing work an
    add does not do and it stays.
    """
    read = {value.id for block in body.blocks for other in block.ops if other is not op for value in other.uses}
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


def _woven(block, ahead: list, behind: list, replacements: dict[int, Op]) -> list:
    """The block with the new counter set up and advanced, and the multiply out.

    `ahead` goes at the end of the preheader, after everything it may read.
    `behind` goes before whatever leaves the latch, because a branch reads
    the flags something before it set and a new add would be read as having
    changed them.
    """
    kept = [replacements.get(id(op), op) for op in block.ops]
    if ahead or behind:
        cut = len(kept)
        while cut and kept[cut - 1].kind in (mir.Kind.JUMP, mir.Kind.BRANCH):
            cut -= 1
        if cut < len(kept):
            at = kept[cut].covers[0] if kept[cut].covers else kept[cut].at
            boundary = at
        elif kept:
            at = kept[-1].at
            boundary = kept[-1].covers[1] if kept[-1].covers else at
        else:
            at = block.at
            boundary = at
        inserted = [replace(op, at=at, covers=(boundary, boundary)) for op in (*ahead, *behind)]
        kept = kept[:cut] + inserted + kept[cut:]
    return kept


def _made(kind, name: str, into, args: tuple, at: int, beside: Op) -> Op:
    """One operation this pass invented, claiming none of BC's bytes."""
    loads = tuple(one.ref for one in args if isinstance(one, mir.Cell))
    uses = dict.fromkeys(one.value for one in args if isinstance(one, mir.Held))
    uses.update((value, None) for ref in loads for value in (ref.base, ref.segment) if value is not None)
    return Op(
        at=at,
        op=beside.op,
        name=name,
        defines=(into,),
        uses=tuple(uses),
        loads=loads,
        stores=(),
        node=None,
        made=None,
        covers=(at, at),
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
