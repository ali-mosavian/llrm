"""Which values are affine functions of a loop's counter.

LLVM's `ScalarEvolution`, in the one shape this needs: a value is either a
constant in the loop, or `start + step * iteration` -- an *affine
recurrence*, `{start,+,step}` in LLVM's own notation. Everything a loop
does to an array index is that.

Split from the transform on purpose. `LoopStrengthReduce` asks this and
rewrites; `IndVarSimplify` asks the same thing and does something else; and
a measurement asks it and does nothing at all. An analysis that changes
nothing can be run by all three.

**Why it matters here more than anything else.** `docs/targets.md` names
what closes each program's gap, and induction variables and their strength
reduction come up in six of the thirteen -- more than any other item.
`stride` is "a division that is really a counter", `matrix` wants a stride
of 2(w+1) on the address, `harr` wants "one add, not a multiply and two
segment loads". BC recomputes an element's address from the index every
time round, because it compiles a statement at a time.
"""

from dataclasses import dataclass

from qbopt import loops as loopy
from qbopt import mir


@dataclass(frozen=True, slots=True)
class Affine:
    """`start + step * iteration`, in the loop this was asked about.

    `start` is the value on the way in and `step` what is added each time
    round. Both are MIR operands: a step of `Const(2)` walks a word array,
    and a step of `Held(w)` walks a row of a matrix whose width the loop
    does not change.
    """

    variable: int  # which MIR variable carries it
    start: "mir.Arg"
    step: "mir.Arg"
    header: int  # the loop it recurs in


@dataclass(frozen=True, slots=True)
class Derived:
    """An operation that computes an affine value from another one.

    `by` is what the recurrence is multiplied by -- loop-invariant, or
    there would be no recurrence to reduce. The point of finding it is that
    `i * by` recomputed every iteration is `j += step * by` accumulated,
    which is an add where BC wrote a multiply.
    """

    op: "mir.Op"
    of: Affine
    by: "mir.Arg"


def invariant(body: mir.MirBody, inside: set[int]) -> set[int]:
    """Every value no operation inside the loop defines."""
    written = {
        value.id
        for block in body.blocks
        if block.at in inside
        for op in block.ops
        for value in op.defines
    }
    written |= {
        phi.result.id for block in body.blocks if block.at in inside for phi in block.phis
    }
    return {
        value.id
        for block in body.blocks
        for op in block.ops
        for value in (*op.defines, *op.uses)
        if value.id not in written
    }


def basics(body: mir.MirBody, loop) -> dict[int, Affine]:
    """Every counter this loop advances by a fixed amount, by variable.

    A header phi whose loop arm is that phi's own result plus something the
    loop does not change. LLVM would say the phi is an `AddRec`; the shape
    is the same and the recognition is the same walk.
    """
    at_of = {block.at: block for block in body.blocks}
    inside = {at for at in loop.body if at in at_of}
    header = at_of.get(loop.header)
    if header is None:
        return {}
    still = invariant(body, inside)
    made = {value.id: op for at in inside for op in at_of[at].ops for value in op.defines}

    out: dict[int, Affine] = {}
    for phi in header.phis:
        start = next(
            (value for where, value in phi.incoming.items() if where not in inside), None
        )
        if start is None:
            continue
        for where, value in phi.incoming.items():
            if where not in inside:
                continue
            step = _stepped(made.get(value.id), phi.result.variable, still)
            if step is not None:
                out[phi.result.variable] = Affine(
                    phi.result.variable, mir.Held(start, _width(start)), step, loop.header
                )
    return out


def _stepped(op: "mir.Op | None", variable: int, still: set[int]) -> "mir.Arg | None":
    """What this operation adds to `variable` each time round, or None."""
    if op is None or op.kind not in (mir.Kind.ADD, mir.Kind.SUB) or op.loads or op.stores:
        return None
    itself = [
        one for one in op.args if isinstance(one, mir.Held) and one.value.variable == variable
    ]
    other = [one for one in op.args if one not in itself]
    if len(itself) != 1 or len(other) != 1:
        return None
    step = other[0]
    if isinstance(step, mir.Const):
        return step if op.kind is mir.Kind.ADD else mir.Const(-step.n, step.width)
    if isinstance(step, mir.Held) and step.value.id in still and op.kind is mir.Kind.ADD:
        return step
    return None


def unwritten(
    body: mir.MirBody, inside: set[int], dgroup: frozenset[int], bounds: dict | None = None
) -> "callable":
    """Whether a cell is one no store inside the loop can reach.

    A value's invariance is a question about definitions; a cell's is a
    question about aliasing, and asking only the first missed the case BC
    actually writes. It reads a multiplier straight out of memory --
    `imul word [w]` -- so 23 of the corpus's 28 counter-multiplies had a
    `Cell` where this looked for a value, and none of them qualified.

    `bounds` is what makes the answer usable rather than merely honest. An
    indexed store into an array -- `[seg:5+si+6]` -- may land anywhere in
    the segment unless something says how big the array is, and then every
    scalar in DGROUP reads as written by it. `module.landmarks()` knows
    where each object ends.
    """
    wrote = [
        one
        for block in body.blocks
        if block.at in inside
        for op in block.ops
        for one in op.stores
    ]

    def settled(cell: "mir.MemRef") -> bool:
        return not any(mir.overlapping(cell, one, dgroup, bounds) for one in wrote)

    return settled


def derived(
    body: mir.MirBody,
    loop,
    found: dict[int, Affine] | None = None,
    dgroup: frozenset[int] = frozenset(),
    bounds: dict | None = None,
) -> list[Derived]:
    """Every multiply inside the loop whose operand is one of its counters.

    The strength reduction candidates: `i * w` recomputed every iteration,
    where `w` does not change. A shift is a multiply written shorter and is
    the same candidate.
    """
    at_of = {block.at: block for block in body.blocks}
    inside = {at for at in loop.body if at in at_of}
    found = found if found is not None else basics(body, loop)
    if not found:
        return []
    still = invariant(body, inside)
    settled = unwritten(body, inside, dgroup, bounds)

    out = []
    for at in inside:
        for op in at_of[at].ops:
            # It may load: the multiplier is a Cell and `imul word [w]`
            # reads it. Excluding anything that touches memory excluded
            # every one of the 23 sites this pass exists for.
            if op.kind not in (mir.Kind.MUL, mir.Kind.SHL) or op.stores:
                continue
            counter = [
                one
                for one in op.args
                if isinstance(one, mir.Held) and one.value.variable in found
            ]
            other = [one for one in op.args if one not in counter]
            if len(counter) != 1 or len(other) != 1:
                continue
            by = other[0]
            if isinstance(by, mir.Held) and by.value.id not in still:
                continue
            if isinstance(by, mir.Cell) and not settled(by.ref):
                continue
            out.append(Derived(op, found[counter[0].value.variable], by))
    return out


def _width(value) -> int:
    """The width a value is carried at, which MIR does not say directly."""
    return 4 if getattr(value, "wide", False) else 2


def of(
    body: mir.MirBody, dgroup: frozenset[int] = frozenset(), bounds: dict | None = None
) -> list[tuple[object, dict[int, Affine], list[Derived]]]:
    """Every loop in this body, with its counters and what they derive."""
    out = []
    for loop in loopy.loops(list(body.blocks), body.entry):
        found = basics(body, loop)
        if found:
            out.append((loop, found, derived(body, loop, found, dgroup, bounds)))
    return out
