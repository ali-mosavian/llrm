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

from math import gcd
from dataclasses import dataclass
from collections.abc import Callable

from qbopt.model import mir
from qbopt.analysis import consts
from qbopt.analysis import loops as loopy


@dataclass(frozen=True, slots=True)
class Affine:
    """`start + step * iteration`, in the loop this was asked about.

    `start` is the value on the way in and `step` what is added each time
    round. Both are MIR operands: a step of `Const(2)` walks a word array,
    and a step of `Held(w)` walks a row of a matrix whose width the loop
    does not change.
    """

    value: int  # the exact SSA value at the loop header
    start: mir.Held | mir.Const
    step: mir.Held | mir.Const
    header: int  # the loop it recurs in


@dataclass(frozen=True, slots=True)
class Derived:
    """An operation that computes an affine value from another one.

    `by` is what the recurrence is multiplied by -- loop-invariant, or
    there would be no recurrence to reduce. The point of finding it is that
    `i * by` recomputed every iteration is `j += step * by` accumulated,
    which is an add where BC wrote a multiply.

    `offsets` are invariant terms with integer coefficients, added to the
    scaled counter. They affect initialization, never the recurrence step.
    `pointer` denotes an invariant pointer advanced by that byte recurrence.
    """

    op: "mir.Op"
    of: Affine
    by: "mir.Arg"
    offsets: tuple[tuple[mir.Arg, int], ...] = ()
    pointer: mir.Arg | None = None


@dataclass(frozen=True, slots=True)
class AffineMap:
    """A width-limited ``scale * source + offset`` relation.

    Equality may be transferred through the map only while it is injective
    over the complete source domain.  Keeping the modular period with the
    relation prevents formula selection and induction simplification from
    growing separate, subtly different overflow proofs.
    """

    scale: int
    offset: int
    width: int

    @property
    def period(self) -> int:
        return (1 << (self.width * 8)) // gcd(abs(self.scale), 1 << (self.width * 8))

    def injective(self, low: int, high: int) -> bool:
        return self.scale != 0 and high - low < self.period


@dataclass(frozen=True, slots=True)
class LoopShape:
    """The canonical pre-tested, single-latch loop CFG.

    This is control-flow structure only.  Keeping it separate from counted
    loop semantics prevents every consumer from spelling its own subtly
    different preheader, latch, entry, and exit recognizer.
    """

    preheader: int
    latch: int
    entered: int
    exit: int


@dataclass(frozen=True, slots=True)
class CountedLoop:
    """A canonical zero-or-more loop with an exact symbolic trip count.

    This is deliberately a proof object rather than another recognizer in a
    transform.  ``trip_count`` answers the narrower question "is the count a
    positive host integer?"; this object retains the useful answer when the
    count is an invariant MIR value:

        i = start; while i test bound: ...; i += 1

    ``test`` is ``<`` or ``<=``, signed or unsigned. The loop is entered iff
    ``start test bound``, and then runs ``bound - start`` trips, one more
    when inclusive, without assuming a value for either.  Consumers may turn
    the control recurrence into a guarded countdown, but may not infer that
    an unrelated scaled recurrence is injective over that unknown domain.
    """

    counter: Affine
    phi: mir.Phi
    compare: mir.Op
    branch: mir.Op
    start: mir.Held | mir.Const
    bound: mir.Held | mir.Const
    test: mir.Kind  # the comparison that continues the loop
    preheader: int
    latch: int
    entered: int
    exit: int
    maximum: int | None = None

    @property
    def inclusive(self) -> bool:
        return self.test in (mir.Kind.LE, mir.Kind.BELOW_EQ)


_SKIPPED = {
    mir.Kind.BELOW: mir.Kind.BELOW_EQ,
    mir.Kind.LT: mir.Kind.LE,
    mir.Kind.BELOW_EQ: mir.Kind.BELOW,
    mir.Kind.LE: mir.Kind.LT,
}


Computed = Callable[[mir.Kind, tuple[mir.Arg, ...]], mir.Held | mir.Const]


def skipped(proof: CountedLoop) -> tuple[tuple[mir.Held | mir.Const, mir.Held | mir.Const], mir.Kind]:
    """The preheader comparison, and the test on it, under which the loop runs no trips."""
    test = _SKIPPED[proof.test]
    if test is mir.Kind.BELOW_EQ and proof.start == mir.Const(0, proof.start.width):
        test = mir.Kind.EQ  # nothing is below zero
    return (proof.bound, proof.start), test


def trips(proof: CountedLoop, computed: Computed) -> mir.Held | mir.Const:
    """Trips on the entered path, exact modulo the counter's width.

    `computed(kind, args)` places one preheader operation and returns its
    result. `counted` proved the count fits: an exclusive test cannot reach
    the width's size, and an inclusive one is proved finite first.
    """
    width = proof.bound.width
    if isinstance(proof.bound, mir.Const) and isinstance(proof.start, mir.Const):
        return mir.Const(consts.masked(proof.bound.n - proof.start.n + proof.inclusive, width), width)
    count = computed(mir.Kind.SUB, (proof.bound, proof.start))
    return computed(mir.Kind.ADD, (count, mir.Const(int(proof.inclusive), width)))


def exit_value(proof: CountedLoop, computed: Computed) -> mir.Held | mir.Const:
    """The counter as a loop that ran a trip leaves: the first value failing its test."""
    width = proof.bound.width
    if isinstance(proof.bound, mir.Const):
        return mir.Const(consts.masked(proof.bound.n + proof.inclusive, width), width)
    return computed(mir.Kind.ADD, (proof.bound, mir.Const(int(proof.inclusive), width)))


@dataclass(frozen=True, slots=True)
class ControlReplacement:
    """Proof that a counted loop's source recurrence may be removed.

    ``covered`` operations may be replaced by the caller's chosen affine
    formula.  Everything else that observes the counter is rejected here,
    once, including phi edges and flag readers.
    """

    counted: CountedLoop
    stepping: mir.Op
    update: mir.Value
    aliases: frozenset[mir.Value]
    copies: frozenset[int]
    # Exit-block phis reading the counter as the loop leaves: `exit_value`
    # after a trip, `start` after none.
    exits: tuple[mir.Phi, ...] = ()


@dataclass(frozen=True, slots=True)
class ZeroTerminatingControl:
    """Proof that an affine recurrence's update flags end counted control.

    The replacement recurrence starts at ``-trips * step`` and is tested for
    zero after each update.  Its original value must therefore be zero, and
    the complete dynamic trip domain must not reach that recurrence's modular
    period before the intended final update.
    """

    replacement: ControlReplacement
    candidate: Affine
    step: int
    maximum: int
    period: int


def canonical(body: mir.MirBody, loop: loopy.Loop) -> LoopShape | None:
    """The one normalized loop shape consumed by induction transforms."""
    blocks = {block.at: block for block in body.blocks}
    if len(loop.latches) != 1 or loop.header not in blocks:
        return None
    latch_at = next(iter(loop.latches))
    latch = blocks.get(latch_at)
    header = blocks[loop.header]
    inside = set(loop.body)
    outside = [at for at in loopy.predecessors(body.blocks).get(header.at, ()) if at not in inside]
    entered = [at for at in header.succ if at in inside and at != header.at]
    exits = [at for at in header.succ if at not in inside]
    if (
        latch is None
        or len(outside) != 1
        or blocks[outside[0]].succ != (header.at,)
        or latch.succ != (header.at,)
        or len(entered) != 1
        or len(exits) != 1
        or not header.ops
        or header.ops[-1].kind is not mir.Kind.BRANCH
        or any(any(to not in inside for to in blocks[at].succ) for at in inside if at != header.at)
    ):
        return None
    return LoopShape(outside[0], latch_at, entered[0], exits[0])


def counted(body: mir.MirBody, loop: loopy.Loop, facts: dict | None = None) -> tuple[CountedLoop, ...]:
    """Prove every canonical ``start ..< bound`` or ``start ..= bound`` unit control recurrence.

    Loop normalization gives analyses one structural spelling: a dedicated
    preheader, a pre-tested header, one latch, and no side exit.  This proof
    adds the semantic facts which shape alone cannot supply.  It is shared by
    strength reduction and loop rotation so neither pass grows a subtly
    different interpretation of the same branch.

    An exclusive test stops the counter before it can wrap.  An inclusive
    one runs forever where ``bound`` is its type's maximum, so it is proved
    only where that cannot happen: a constant below it, or a finite
    ``maximum``.
    """
    facts = consts.known(body) if facts is None else facts
    blocks = {block.at: block for block in body.blocks}
    shape = canonical(body, loop)
    if shape is None:
        return ()
    header = blocks[loop.header]
    inside = set(loop.body)
    branch = header.ops[-1]
    test = _continuing_test(branch, inside)
    if test not in _SKIPPED:
        return ()
    unsigned = test in (mir.Kind.BELOW, mir.Kind.BELOW_EQ)
    inclusive = test in (mir.Kind.LE, mir.Kind.BELOW_EQ)
    still = invariant(body, inside)
    made = {value.id: op for block in body.blocks for op in block.ops for value in op.defines}
    proven = []
    for counter in basics(body, loop).values():
        width = counter.start.width
        if _signed(counter.step, facts, width) != 1 or not isinstance(counter.start, (mir.Held, mir.Const)):
            continue
        phi = next((one for one in header.phis if one.result.id == counter.value), None)
        if phi is None or set(phi.incoming) != {shape.preheader, shape.latch}:
            continue
        comparisons = [
            (op, bound)
            for op in header.ops[:-1]
            if (bound := _counter_bound(op, branch, counter, width, made)) is not None
        ]
        if len(comparisons) != 1:
            continue
        compare, bound = comparisons[0]
        if not isinstance(bound, (mir.Held, mir.Const)) or bound.width != width:
            continue
        if isinstance(bound, mir.Held) and bound.value.id not in still:
            continue
        update = phi.incoming[shape.latch]
        stepping = made.get(update.id)
        if (
            stepping is None
            or mir.stepping(stepping) != (mir.Held(phi.result, width), mir.Const(1, width))
            or stepping.results != (mir.Held(update, width),)
            or stepping.loads
            or stepping.stores
            or stepping.barrier
            or stepping.merges
        ):
            continue
        start = _constant(counter.start, facts, width)
        limit = _constant(bound, facts, width)
        if unsigned:
            first, last, top = start, limit, (1 << 8 * width) - 1
        else:
            first = None if start is None else _as_signed(start, width)
            last = None if limit is None else _as_signed(limit, width)
            top = (1 << 8 * width - 1) - 1
        if inclusive and last == top:
            continue
        lowest = first if first is not None else _range(body, counter.start, 0, top)
        highest = last if last is not None else _range(body, bound, 1, top - inclusive)
        maximum = None
        if lowest is not None and highest is not None:
            maximum = max(0, highest - lowest + inclusive)
        if maximum is None:
            maximum = _inbounds_trips(body, loop, shape.latch)
        if inclusive and last is None and maximum is None:
            continue
        proven.append(
            CountedLoop(
                counter,
                phi,
                compare,
                branch,
                counter.start if start is None else mir.Const(start, width),
                bound if limit is None else mir.Const(limit, width),
                test,
                shape.preheader,
                shape.latch,
                shape.entered,
                shape.exit,
                maximum,
            )
        )
    return tuple(proven)


def advances(body: mir.MirBody, loop: loopy.Loop) -> dict[mir.Value, int]:
    """How far each counter and each value affine in one advances per iteration.

    The bytes-per-iteration view of `basics` and `derived`; nothing here
    re-derives which values are affine.
    """
    found = basics(body, loop)
    header = next(block for block in body.blocks if block.at == loop.header)
    out = {
        phi.result: _as_signed(one.step.n, one.step.width)
        for phi in header.phis
        if (one := found.get(phi.result.id)) is not None and isinstance(one.step, mir.Const)
    }
    for one in derived(body, loop, found):
        if (
            isinstance(one.of.step, mir.Const)
            and isinstance(one.by, mir.Const)
            and one.pointer is None
            and len(one.op.results) == 1
            and isinstance(one.op.results[0], mir.Held)
        ):
            out[one.op.results[0].value] = _as_signed(one.of.step.n, one.of.step.width) * _as_signed(
                one.by.n, one.by.width
            )
    return {value: step for value, step in out.items() if step}


def _range(body: mir.MirBody, arg: mir.Arg, end: int, top: int) -> int | None:
    """The low (`end` 0) or high (`end` 1) of a frontend range on `arg`, if it lies in `0 ..= top`."""
    interval = body.integer_ranges.get(arg.value) if isinstance(arg, mir.Held) else None
    if interval is None or interval.width != arg.width or interval.low < 0 or interval.high > top:
        return None
    return (interval.low, interval.high)[end]


def _inbounds_trips(body: mir.MirBody, loop: loopy.Loop, latch: int) -> int | None:
    """The most iterations an access made every iteration allows, as LLVM's inbounds does.

    Iteration i reaches `b + i*s` inside one object, and an offset `w` bytes
    wide addresses at most 2**(8w) of them, so i*s + width <= 2**(8w).
    """
    step = advances(body, loop)
    every = loopy.dominators(body.blocks, body.entry).get(latch, frozenset())
    limits = [
        ((1 << 8 * ref.base_width) - ref.width) // abs(step[ref.base]) + 1
        for block in body.blocks
        # The header also runs the final, failing test: n + 1 times.
        if block.at in loop.body and block.at in every and block.at != loop.header
        for op in block.ops
        for ref in (*op.loads, *op.stores)
        if ref.base in step
    ]
    return min(limits, default=None)


def transparent_aliases(
    body: mir.MirBody,
    loop: loopy.Loop,
    source: mir.Value,
) -> tuple[frozenset[mir.Value], frozenset[int]]:
    """Values and operations in a width-preserving copy chain."""
    aliases = {source}
    copies: set[int] = set()
    changed = True
    while changed:
        changed = False
        for block in body.blocks:
            if block.at not in loop.body:
                continue
            for op in block.ops:
                if (
                    op.kind is mir.Kind.COPY
                    and len(op.args) == len(op.results) == 1
                    and isinstance(op.args[0], mir.Held)
                    and isinstance(op.results[0], mir.Held)
                    and op.args[0].value in aliases
                    and op.results[0].width == op.args[0].width
                ):
                    copies.add(id(op))
                    if op.results[0].value not in aliases:
                        aliases.add(op.results[0].value)
                        changed = True
    return frozenset(aliases), frozenset(copies)


def control_replacement(
    body: mir.MirBody,
    loop: loopy.Loop,
    proof: CountedLoop,
    covered: frozenset[int] = frozenset(),
) -> ControlReplacement | None:
    """Prove that ``covered``, loop control and exit phis are every counter use."""
    blocks = {block.at: block for block in body.blocks}
    predecessors = loopy.predecessors(body.blocks)
    header, latch = blocks[loop.header], blocks[proof.latch]
    if (
        len(loop.body) != 2
        or proof.entered != proof.latch
        or latch.phis
        or set(predecessors.get(latch.at, ())) != {header.at}
        or any(op is not proof.compare and op is not proof.branch and not test_only(op) for op in header.ops)
    ):
        return None
    made = {value.id: op for block in body.blocks for op in block.ops for value in op.defines}
    update = proof.phi.incoming[proof.latch]
    stepping = made.get(update.id)
    if stepping is None:
        return None
    aliases, copies = transparent_aliases(body, loop, proof.phi.result)
    allowed = covered | copies | {id(proof.compare), id(stepping)}
    if any(
        (aliases.intersection(op.uses) and id(op) not in allowed) or update in op.uses
        for block in body.blocks
        for op in block.ops
    ):
        return None
    exits = tuple(other for other in blocks[proof.exit].phis if other.incoming == {header.at: proof.phi.result})
    if any(
        other is not proof.phi and other not in exits and ({*aliases, update} & set(other.incoming.values()))
        for block in body.blocks
        for other in block.phis
    ):
        return None
    compare_flags = {value for value in proof.compare.defines if value.flags}
    step_flags = {value for value in stepping.defines if value.flags}
    if any(
        (compare_flags.intersection(op.uses) and op is not proof.branch) or step_flags.intersection(op.uses)
        for block in body.blocks
        for op in block.ops
    ):
        return None
    return ControlReplacement(proof, stepping, update, aliases, copies, exits)


def zero_terminating_control(
    body: mir.MirBody,
    loop: loopy.Loop,
    proof: CountedLoop,
    candidate: Affine,
    facts: dict | None = None,
) -> ZeroTerminatingControl | None:
    """Prove that ``candidate`` can supply a counted loop's terminating flags.

    Replacing counted control with an existing affine recurrence seeds it at
    ``-trips * step`` and rebases its users by its final value, so its final
    update is zero whatever it started from.  Bounded period safety excludes
    an earlier modular zero.  This proof belongs here because it is
    independent of the transform's choice of which address expressions to
    rebase.
    """
    replacement = control_replacement(body, loop, proof)
    if replacement is None or proof.maximum is None or candidate == proof.counter:
        return None
    width = proof.counter.start.width
    if candidate.start.width != width or candidate.step.width != width or proof.maximum < 0:
        return None
    facts = consts.known(body) if facts is None else facts
    step = _signed(candidate.step, facts, width)
    if step in (None, 0):
        return None
    assert step is not None
    period = AffineMap(step, 0, width).period
    if proof.maximum > period:
        return None
    return ZeroTerminatingControl(replacement, candidate, step, proof.maximum, period)


def test_only(op: mir.Op) -> bool:
    """Whether skipping ``op`` skips no value or side effect."""
    if (
        op.kind is mir.Kind.NOTHING
        and not op.name
        and not (op.defines or op.uses or op.args or op.results or op.loads or op.stores or op.merges or op.barrier)
        and op.floating is None
        and op.stack is None
        and op.floating_origin is None
    ):
        return True
    return (
        op.kind in (mir.Kind.SUB, mir.Kind.AND, mir.Kind.OR)
        and not (op.results or op.loads or op.stores or op.merges or op.barrier)
        and op.floating is None
        and op.stack is None
        and op.floating_origin is None
        and bool(op.defines)
        and all(value.flags for value in op.defines)
    )


def relation(source: Affine, target: Affine, facts: dict) -> AffineMap | None:
    """The constant modular affine map from ``source`` to ``target``."""
    width = source.start.width
    if target.start.width != width:
        return None
    source_start = _signed(source.start, facts, width)
    source_step = _signed(source.step, facts, width)
    target_start = _signed(target.start, facts, width)
    target_step = _signed(target.step, facts, width)
    if None in (source_start, source_step, target_start, target_step) or not source_step:
        return None
    assert source_start is not None and source_step is not None and target_start is not None and target_step is not None
    if target_step % source_step:
        return None
    scale = target_step // source_step
    if not scale:
        return None
    mask = (1 << (width * 8)) - 1
    return AffineMap(scale, (target_start - scale * source_start) & mask, width)


def derived_map(formula: Derived, facts: dict) -> AffineMap | None:
    """The constant affine map represented by a derived formula."""
    width = formula.of.start.width
    scale = _signed(formula.by, facts, width)
    if scale in (None, 0) or formula.pointer is not None:
        return None
    modulus = 1 << (width * 8)
    offset = 0
    for value, coefficient in formula.offsets:
        constant = _constant(value, facts, width)
        if constant is None:
            return None
        offset = (offset + constant * coefficient) % modulus
    return AffineMap(scale, offset, width)


def domain(body: mir.MirBody, loop: loopy.Loop, affine: Affine, facts: dict) -> tuple[int, int] | None:
    """The finite inclusive integer domain visited by ``affine``."""
    width = affine.start.width
    start = _signed(affine.start, facts, width)
    last = _last_counter(body, loop, affine, facts, width)
    return None if start is None or last is None else (min(start, last), max(start, last))


def invariant(body: mir.MirBody, inside: set[int]) -> set[int]:
    """Every value no operation inside the loop defines."""
    written = {value.id for block in body.blocks if block.at in inside for op in block.ops for value in op.defines}
    written |= {phi.result.id for block in body.blocks if block.at in inside for phi in block.phis}
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
        starts = [value for where, value in phi.incoming.items() if where not in inside]
        if not starts or any(value != starts[0] for value in starts):
            continue
        steps = []
        widths = set()
        for where, value in phi.incoming.items():
            if where not in inside:
                continue
            definition = made.get(value.id)
            results = (
                [arg for arg in definition.results if isinstance(arg, mir.Held) and arg.value == value]
                if definition
                else []
            )
            if len(results) != 1:
                steps.append(None)
                continue
            width = results[0].width
            widths.add(width)
            root = _copied(results[0], made)
            step = _stepped(made.get(root.value.id), phi.result.id, still, made)
            steps.append(step if step is not None and step.width == width else None)
        if len(widths) == 1 and steps and steps[0] is not None and all(step == steps[0] for step in steps):
            out[phi.result.id] = Affine(phi.result.id, mir.Held(starts[0], widths.pop()), steps[0], loop.header)
    return out


def _copied(operand: mir.Held, made: dict[int, mir.Op]) -> mir.Held:
    seen = set()
    while operand.value.id not in seen:
        seen.add(operand.value.id)
        op = made.get(operand.value.id)
        if op is None or op.kind is not mir.Kind.COPY or op.loads or op.stores:
            break
        if len(op.args) != 1 or len(op.results) != 1:
            break
        source, result = op.args[0], op.results[0]
        if not isinstance(source, mir.Held) or not isinstance(result, mir.Held):
            break
        if source.width != operand.width or result.width != operand.width:
            break
        operand = source
    return operand


def _stepped(op: mir.Op | None, value: int, still: set[int], made: dict[int, mir.Op]) -> mir.Held | mir.Const | None:
    """What this operation adds to `variable` each time round, or None."""
    if op is None:
        return None
    # `mir.stepping` is the one place that says what an operation adds to
    # what: `add`, `sub`, and the two that carry their operand in the
    # opcode all answer it, and nothing here lists kinds.
    got = mir.stepping(op)
    if got is None:
        return None
    stepped, step = got
    if isinstance(stepped, mir.Held):
        stepped = _copied(stepped, made)
    if isinstance(step, mir.Held):
        step = _copied(step, made)
    if not isinstance(stepped, mir.Held) or stepped.value.id != value:
        # An `add` may name the counter second; the two that step by one
        # never do.
        if isinstance(step, mir.Held) and step.value.id == value:
            stepped, step = step, stepped
        else:
            return None
    if isinstance(step, mir.Const):
        return step
    if isinstance(step, mir.Held) and step.value.id in still:
        return step
    return None


def unwritten(body: mir.MirBody, inside: set[int], dgroup: frozenset[int], bounds: dict | None = None) -> "callable":
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
    from qbopt.analysis import ranges

    wrote = [one for block in body.blocks if block.at in inside for op in block.ops for one in op.stores]
    # With constants, as hoist asks: a store through the literal selector
    # 0A000h otherwise lands on the frame and on every array descriptor.
    known = ranges.constants(body, dgroup) if wrote else {}

    def settled(cell: "mir.MemRef") -> bool:
        return not any(mir.overlapping(cell, one, dgroup, bounds, known=known, other_known=known) for one in wrote)

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
    made = {value.id: op for block in body.blocks for op in block.ops for value in op.defines}

    out = []
    for at in inside:
        for op in at_of[at].ops:
            if (
                op.kind is mir.Kind.PTR_OFFSET
                and len(op.args) == 2
                and len(op.results) == 1
                and isinstance(op.results[0], mir.Held)
                and op.results[0].width == 4
                and not op.loads
                and not op.stores
                and not op.barrier
                and not op.merges
            ):
                pointer, offset = op.args
                if (
                    isinstance(pointer, mir.Held)
                    and pointer.value.id in still
                    and isinstance(offset, mir.Held)
                    and offset.value.id in found
                    and pointer.width == offset.width == found[offset.value.id].start.width == 4
                ):
                    out.append(Derived(op, found[offset.value.id], mir.Const(1, 4), pointer=pointer))
                continue
            # It may load: the multiplier is a Cell and `imul word [w]`
            # reads it. Excluding anything that touches memory excluded
            # every one of the 23 sites this pass exists for.
            if op.kind not in (mir.Kind.MUL, mir.Kind.SHL) or op.stores:
                continue
            args = tuple(_copied(one, made) if isinstance(one, mir.Held) else one for one in op.args)
            counter = [one for one in args if isinstance(one, mir.Held) and one.value.id in found]
            other = [one for one in args if one not in counter]
            if len(counter) != 1 or len(other) != 1:
                continue
            by = other[0]
            if op.kind is mir.Kind.SHL and (
                args[0] != counter[0] or not isinstance(by, mir.Const) or not 0 <= by.n < counter[0].width * 8
            ):
                continue
            if isinstance(by, mir.Held) and by.value.id not in still:
                continue
            if isinstance(by, mir.Cell) and not settled(by.ref):
                continue
            out.append(Derived(op, found[counter[0].value.id], _multiplier(op, by)))
    combined = {id(one.op): one for one in out}
    combined.update({id(one.op): one for one in _composed(body, loop, found, made, settled)})
    combined.update({id(one.op): one for one in _quotients(body, loop, found)})
    return list(combined.values())


def _signed(arg: mir.Arg, facts: dict, width: int) -> int | None:
    if not isinstance(arg, (mir.Held, mir.Const)) or arg.width != width:
        return None
    fact = facts.get(arg.value) if isinstance(arg, mir.Held) else arg
    if fact is None or fact.width < width:
        return None
    sign = 1 << (width * 8 - 1)
    return ((fact.n & (sign * 2 - 1)) ^ sign) - sign


def _last_counter(body: mir.MirBody, loop, counter: Affine, facts: dict, width: int) -> int | None:
    """Last executed counter of a canonical pretested loop, proving its update cannot wrap."""
    posttested = _posttested_last(body, loop, counter, facts, width)
    if posttested is not None:
        return posttested
    blocks = {block.at: block for block in body.blocks}
    if len(loop.latches) != 1:
        return None
    header, latch = blocks[loop.header], blocks[next(iter(loop.latches))]
    if latch.succ != (header.at,) or not header.ops or len(header.succ) != 2:
        return None
    branch = header.ops[-1]
    if branch.kind is not mir.Kind.BRANCH or branch.target not in header.succ:
        return None
    inside = set(loop.body)
    if any(not blocks[at].succ or any(to not in inside for to in blocks[at].succ) for at in inside if at != header.at):
        return None
    if sum(to in inside for to in header.succ) != 1:
        return None
    test = _continuing_test(branch, inside)
    if test is None:
        return None
    made = {value.id: op for block in body.blocks for op in block.ops for value in op.defines}
    comparisons = [
        bound for op in header.ops[:-1] if (bound := _counter_bound(op, branch, counter, width, made)) is not None
    ]
    if len(comparisons) != 1:
        return None
    raw = tuple(_constant(arg, facts, width) for arg in (counter.start, counter.step, comparisons[0]))
    if any(value is None for value in raw):
        return None
    raw_start, raw_step, raw_bound = raw
    assert raw_start is not None and raw_step is not None and raw_bound is not None
    step = _as_signed(raw_step, width)
    if step == 0:
        return None
    unsigned = test in (mir.Kind.BELOW, mir.Kind.BELOW_EQ, mir.Kind.ABOVE, mir.Kind.ABOVE_EQ)
    start = raw_start if unsigned else _as_signed(raw_start, width)
    bound = raw_bound if unsigned else _as_signed(raw_bound, width)
    if step > 0 and test in (mir.Kind.LE, mir.Kind.LT, mir.Kind.BELOW_EQ, mir.Kind.BELOW):
        limit = bound - (test is mir.Kind.LT)
        if test is mir.Kind.BELOW:
            limit = bound - 1
        distance = limit - start
    elif step < 0 and test in (mir.Kind.GE, mir.Kind.GT, mir.Kind.ABOVE_EQ, mir.Kind.ABOVE):
        limit = bound + (test is mir.Kind.GT)
        if test is mir.Kind.ABOVE:
            limit = bound + 1
        distance = start - limit
    elif test is mir.Kind.NE and (bound - start) * step > 0 and (bound - start) % step == 0:
        distance = abs(bound - start) - abs(step)
    else:
        return None
    if distance < 0:
        return None
    last = start + (distance // abs(step)) * step
    after = last + step
    if unsigned:
        return last if 0 <= after < 1 << (width * 8) else None
    sign = 1 << (width * 8 - 1)
    return last if -sign <= after < sign else None


def _continuing_test(branch: mir.Op, inside: set[int]) -> mir.Kind | None:
    """The condition under which this branch keeps executing its loop.

    Conditional branches name the taken edge, while recurrence reasoning
    names the edge that returns to the header.  Keeping that inversion here
    lets pre-tested and rotated post-tested loops share the exact same
    comparison semantics.
    """
    if branch.target in inside:
        return branch.test
    return {
        mir.Kind.LE: mir.Kind.GT,
        mir.Kind.LT: mir.Kind.GE,
        mir.Kind.GE: mir.Kind.LT,
        mir.Kind.GT: mir.Kind.LE,
        mir.Kind.BELOW: mir.Kind.ABOVE_EQ,
        mir.Kind.BELOW_EQ: mir.Kind.ABOVE,
        mir.Kind.ABOVE: mir.Kind.BELOW_EQ,
        mir.Kind.ABOVE_EQ: mir.Kind.BELOW,
        mir.Kind.EQ: mir.Kind.NE,
        mir.Kind.NE: mir.Kind.EQ,
    }.get(branch.test)


def _posttested_bound(op, branch, counter, width, made):
    """Bound and final update for a post-tested affine counter, if exact.

    Rotation puts a counter's update before its exit comparison.  The compare
    therefore names the next phi value rather than the header value.  This is
    not a special case for an ``inc`` spelling: ``mir.stepping`` supplies the
    mathematical update for every MIR operation that can be an affine step.
    """
    if (
        len(op.args) != 2
        or op.kind is not mir.Kind.SUB
        or op.loads
        or op.stores
        or op.barrier
        or op.results
        or len(op.defines) != 1
        or not isinstance(op.args[0], mir.Held)
        or op.args[0].width != width
        or not any(value.flags and value in branch.uses for value in op.defines)
    ):
        return None
    following = op.args[0]
    definition = made.get(following.value.id)
    if (
        definition is None
        or definition.loads
        or definition.stores
        or definition.barrier
        or definition.merges
        or following not in definition.results
    ):
        return None
    stepped = mir.stepping(definition)
    if stepped is None:
        return None
    source, delta = stepped
    if (
        not isinstance(source, mir.Held)
        or not isinstance(delta, mir.Const)
        or source.width != width
        or delta.width != width
        or _copied(source, made).value.id != counter.value
    ):
        return None
    return op.args[1], delta


def _posttested_last(body: mir.MirBody, loop, counter: Affine, facts: dict, width: int) -> int | None:
    """Last header value of a canonical rotated loop, with a non-wrapping exit.

    A post-tested loop executes its body once before the test.  We only prove
    the compact canonical form: one latch updates the header recurrence,
    compares that immediate next value, and either returns to the header or
    leaves the loop.  Any early exit, extra latch edge, unknown bound, or
    potentially wrapping update remains deliberately unmeasured.
    """
    blocks = {block.at: block for block in body.blocks}
    if len(loop.latches) != 1:
        return None
    latch = blocks[next(iter(loop.latches))]
    inside = set(loop.body)
    if (
        len(latch.succ) != 2
        or loop.header not in latch.succ
        or sum(to in inside for to in latch.succ) != 1
        or not latch.ops
    ):
        return None
    branch = latch.ops[-1]
    if branch.kind is not mir.Kind.BRANCH or branch.target not in latch.succ:
        return None
    if any(any(to not in inside for to in blocks[at].succ) for at in inside if at != latch.at):
        return None
    test = _continuing_test(branch, inside)
    if test is None:
        return None
    made = {value.id: op for block in body.blocks for op in block.ops for value in op.defines}
    comparisons = [
        bound for op in latch.ops[:-1] if (bound := _posttested_bound(op, branch, counter, width, made)) is not None
    ]
    if len(comparisons) != 1:
        return None
    bound_arg, after_step = comparisons[0]
    raw = tuple(_constant(arg, facts, width) for arg in (counter.start, counter.step, bound_arg, after_step))
    if any(value is None for value in raw):
        return None
    raw_start, raw_step, raw_bound, raw_after = raw
    assert raw_start is not None and raw_step is not None and raw_bound is not None and raw_after is not None
    step = _as_signed(raw_step, width)
    after = _as_signed(raw_after, width)
    if step == 0 or after != step:
        return None
    unsigned = test in (mir.Kind.BELOW, mir.Kind.BELOW_EQ, mir.Kind.ABOVE, mir.Kind.ABOVE_EQ)
    start = raw_start if unsigned else _as_signed(raw_start, width)
    bound = raw_bound if unsigned else _as_signed(raw_bound, width)
    first = start + step
    if step > 0 and test in (mir.Kind.LT, mir.Kind.BELOW):
        count = max(1, (bound - first) // step + 1) if first <= bound else 1
    elif step > 0 and test in (mir.Kind.LE, mir.Kind.BELOW_EQ):
        count = max(1, (bound - first) // step + 2) if first <= bound else 1
    elif step < 0 and test in (mir.Kind.GT, mir.Kind.ABOVE):
        count = max(1, (first - bound) // -step + 1) if first >= bound else 1
    elif step < 0 and test in (mir.Kind.GE, mir.Kind.ABOVE_EQ):
        count = max(1, (first - bound) // -step + 2) if first >= bound else 1
    elif test is mir.Kind.NE and (bound - start) * step > 0 and (bound - start) % step == 0:
        count = (bound - start) // step
    else:
        return None
    if count <= 0:
        return None
    last = start + (count - 1) * step
    next_value = last + step
    if unsigned:
        return last if 0 <= last < 1 << (width * 8) and 0 <= next_value < 1 << (width * 8) else None
    sign = 1 << (width * 8 - 1)
    return last if -sign <= last < sign and -sign <= next_value < sign else None


def trip_count(body: mir.MirBody, loop: loopy.Loop, facts: dict) -> int | None:
    """The one proven positive execution count shared by every loop counter.

    A loop may carry an integer counter, a byte address and one or more
    derived counters at once.  They are evidence for the same trip count,
    not alternatives from which a transform may pick the convenient one.
    Refusing disagreement keeps cloning transforms independent of which
    recurrence happened to be visited first.
    """
    counts = set()
    for counter in basics(body, loop).values():
        width = counter.start.width
        start = _signed(counter.start, facts, width)
        step = _signed(counter.step, facts, width)
        last = _last_counter(body, loop, counter, facts, width)
        if start is not None and step and last is not None:
            distance = last - start
            if not distance % step:
                count = distance // step + 1
                if count > 0:
                    counts.add(count)
        symbolic = _sentinel_trip_count(body, loop, counter, facts, width)
        if symbolic is not None:
            counts.add(symbolic)
    if len(counts) != 1:
        # A semantics-preserving loop transform may consume the syntactic
        # relationship which established this fact.  Nested-recurrence
        # rewind, for example, replaces ``start`` with an outer phi after it
        # has proved the inner loop's exact distance.  Retain that proof at
        # the same header so rotation and measurement do not fall back to a
        # guessed trip count.  A newly derived disagreement is still refused.
        return dict(body.loop_trip_counts).get(loop.header) if not counts else None
    return next(iter(counts))


def _sentinel_trip_count(body: mir.MirBody, loop: loopy.Loop, counter: Affine, facts: dict, width: int) -> int | None:
    """Trips to an invariant ``start + step * count`` equality sentinel.

    The start itself need not be constant.  The modular period proves both
    that the sentinel is reached and that it cannot be reached earlier.  This
    is the form left when indvar simplification reuses a pointer or coordinate
    recurrence and removes a narrower constant counter.
    """
    blocks = {block.at: block for block in body.blocks}
    if len(loop.latches) != 1:
        return None
    header, latch = blocks[loop.header], blocks[next(iter(loop.latches))]
    inside = set(loop.body)
    if latch.succ == (header.at,) and len(header.succ) == 2:
        control = header
        posttested = False
    elif len(latch.succ) == 2 and header.at in latch.succ:
        control = latch
        posttested = True
    else:
        return None
    if not control.ops or sum(to in inside for to in control.succ) != 1:
        return None
    branch = control.ops[-1]
    if branch.kind is not mir.Kind.BRANCH or branch.target not in control.succ:
        return None
    if any(not blocks[at].succ or any(to not in inside for to in blocks[at].succ) for at in inside if at != control.at):
        return None
    test = _continuing_test(branch, inside)
    if test is not mir.Kind.NE:
        return None

    made = {value.id: op for block in body.blocks for op in block.ops for value in op.defines}
    owners = {value.id: block.at for block in body.blocks for op in block.ops for value in op.defines}
    if posttested:
        compared = [
            found
            for op in control.ops[:-1]
            if (found := _posttested_bound(op, branch, counter, width, made)) is not None
        ]
        if len(compared) != 1:
            return None
        bound, after_step = compared[0]
        raw_after = _constant(after_step, facts, width)
    else:
        compared = [
            (found, None)
            for op in control.ops[:-1]
            if (found := _counter_bound(op, branch, counter, width, made)) is not None
        ]
        if len(compared) != 1:
            return None
        bound, _after_step = compared[0]
        raw_after = None
    if not isinstance(bound, mir.Held):
        return None
    definition = made.get(bound.value.id)
    if (
        definition is None
        or owners.get(bound.value.id) in inside
        or definition.kind is not mir.Kind.ADD
        or definition.loads
        or definition.stores
        or definition.barrier
        or definition.merges
        or len(definition.args) != 2
        or len(definition.results) != 1
        or definition.results[0] != bound
    ):
        return None
    starts = [arg for arg in definition.args if arg == counter.start]
    offsets = [arg for arg in definition.args if arg != counter.start]
    if len(starts) != 1 or len(offsets) != 1:
        return None
    raw_step = _constant(counter.step, facts, width)
    delta = _constant(offsets[0], facts, width)
    if raw_step in (None, 0) or delta is None or (posttested and raw_after != raw_step):
        return None
    modulus = 1 << (8 * width)
    divisor = gcd(raw_step, modulus)
    if delta % divisor:
        return None
    period = modulus // divisor
    count = (delta // divisor) * pow(raw_step // divisor, -1, period) % period
    return count or None


def _constant(arg: mir.Arg, facts: dict, width: int) -> int | None:
    """An exact width-limited bit pattern, without imposing signedness."""
    if not isinstance(arg, (mir.Held, mir.Const)) or arg.width != width:
        return None
    fact = facts.get(arg.value) if isinstance(arg, mir.Held) else arg
    if fact is None or fact.width < width:
        return None
    return fact.n & ((1 << (width * 8)) - 1)


def _as_signed(value: int, width: int) -> int:
    sign = 1 << (width * 8 - 1)
    return (value ^ sign) - sign


def _counter_bound(op, branch, counter, width, made=None):
    if (
        len(op.args) != 2
        or op.loads
        or op.stores
        or op.barrier
        or not isinstance(op.args[0], mir.Held)
        or op.args[0].width != width
    ):
        return None
    # A partial numeric result retains part of its destination, but the flags
    # this branch consumes are still exactly those of the width named by the
    # operands.  Rejecting that independent value dependency loses ordinary
    # word `or i,i` loop tests merely because the register model tracks an
    # upper-half merge.
    compared = _copied(op.args[0], made) if made is not None else op.args[0]
    if compared.value.id != counter.value:
        return None
    flags = [value for value in op.defines if value.flags]
    if len(flags) != 1 or flags[0] not in branch.uses:
        return None
    if op.kind is mir.Kind.SUB and not op.results and len(op.defines) == 1:
        return op.args[1]
    if op.kind in (mir.Kind.AND, mir.Kind.OR) and op.args[0] == op.args[1]:
        return mir.Const(0, width)
    return None


def _quotients(body: mir.MirBody, loop, found: dict[int, Affine]) -> list[Derived]:
    """Exact division of a non-wrapping recurrence is another recurrence."""
    facts = consts.known(body)
    out = []
    for block in body.blocks:
        if block.at not in loop.body or block.at == loop.header:
            continue
        for op in block.ops:
            if op.kind is not mir.Kind.DIVMOD or len(op.args) != 2 or len(op.results) != 2:
                continue
            if (
                op.loads
                or op.stores
                or op.barrier
                or not all(isinstance(result, mir.Held) and result.width == 2 for result in op.results)
            ):
                continue
            dividend, divisor = op.args
            if not isinstance(dividend, mir.Held) or dividend.width != 2 or dividend.value.id not in found:
                continue
            counter = found[dividend.value.id]
            start, step, denominator = (_signed(arg, facts, 2) for arg in (counter.start, counter.step, divisor))
            if start is None or step is None or denominator in (None, 0):
                continue
            if start % denominator or step % denominator:
                continue
            last = _last_counter(body, loop, counter, facts, 2)
            if last is None or not all(-32768 <= value // denominator <= 32767 for value in (start, last)):
                continue
            quotient = Affine(
                counter.value,
                mir.Const(start // denominator, 2),
                mir.Const(step // denominator, 2),
                loop.header,
            )
            out.append(Derived(op, quotient, mir.Const(1, 2)))
    return out


def nonempty(body: mir.MirBody, loop) -> bool:
    """A canonical counted loop whose first iteration and finite exit are proven."""
    facts = consts.known(body)
    return trip_count(body, loop, facts) is not None


def _composed(body: mir.MirBody, loop, found: dict[int, Affine], made: dict[int, mir.Op], settled) -> list[Derived]:
    inside = set(loop.body)
    known = consts.known(body)
    still = invariant(body, inside)
    forms = {
        value: (one, 1, ())
        for value, one in found.items()
        if isinstance(one.start, (mir.Held, mir.Const)) and one.start.width in (2, 4)
    }
    out: dict[int, Derived] = {}
    changed = True
    while changed:
        changed = False
        for block in body.blocks:
            if block.at not in inside:
                continue
            for op in block.ops:
                if (
                    block.at != loop.header
                    and op.kind in (mir.Kind.SIGN_EXTEND, mir.Kind.ZERO_EXTEND)
                    and op.results
                    and isinstance(op.results[0], mir.Held)
                    and op.results[0].value.id not in forms
                ):
                    extended = _extended(body, loop, op, forms, known)
                    if extended is not None:
                        forms[op.results[0].value.id] = extended
                        changed = True
                # A pointer offset is the same affine formula carried in a
                # different result type.  The direct recognizer above sees
                # `base + i`; a composed offset such as `base + (i * 2)`
                # reaches this fixed point only after the multiply has put
                # its result in ``forms``.  Keep the invariant pointer and
                # the complete offset formula together so strength reduction
                # can make one pointer recurrence instead of rebuilding its
                # address every trip.
                if (
                    op.kind is mir.Kind.PTR_OFFSET
                    and not op.loads
                    and not op.stores
                    and not op.barrier
                    and not op.merges
                    and len(op.args) == 2
                    and len(op.results) == 1
                ):
                    pointer, offset = op.args
                    result = op.results[0]
                    form = forms.get(offset.value.id) if isinstance(offset, mir.Held) else None
                    if (
                        isinstance(pointer, mir.Held)
                        and pointer.value.id in still
                        and isinstance(offset, mir.Held)
                        and isinstance(result, mir.Held)
                        and pointer.width == offset.width == result.width
                        and form is not None
                        and form[0].start.width == result.width
                    ):
                        counter, scale, offsets = form
                        out[id(op)] = Derived(
                            op,
                            counter,
                            mir.Const(consts.masked(scale, result.width), result.width),
                            offsets,
                            pointer,
                        )
                    continue
                if op.stores or op.barrier or len(op.args) != 2 or not op.results:
                    continue
                if set(op.loads) != {arg.ref for arg in op.args if isinstance(arg, mir.Cell)}:
                    continue
                result = op.results[0]
                if not isinstance(result, mir.Held) or result.width not in (2, 4) or result.value.id in forms:
                    continue
                width = result.width
                args = tuple(_copied(arg, made) if isinstance(arg, mir.Held) else arg for arg in op.args)
                args = tuple(
                    mir.Const(consts.masked(fact.n, width), width)
                    if isinstance(arg, mir.Held)
                    and arg.width == width
                    and arg.value.id not in forms
                    and (fact := known.get(arg.value)) is not None
                    and fact.width >= width
                    else arg
                    for arg in args
                )
                left, right = args
                first = forms.get(left.value.id) if isinstance(left, mir.Held) and left.width == width else None
                second = forms.get(right.value.id) if isinstance(right, mir.Held) and right.width == width else None
                if any(form is not None and form[0].start.width != width for form in (first, second)):
                    continue
                if op.kind in (mir.Kind.AND, mir.Kind.OR) and left == right and first is not None:
                    forms[result.value.id] = first
                    changed = True
                    continue
                if op.kind in (mir.Kind.ADD, mir.Kind.SUB) and first is not None and second is not None:
                    if first[0] != second[0]:
                        continue
                    base = first[0]
                    scale = first[1] + second[1] if op.kind is mir.Kind.ADD else first[1] - second[1]
                    sign = 1 if op.kind is mir.Kind.ADD else -1
                    offsets = first[2] + tuple((arg, coefficient * sign) for arg, coefficient in second[2])
                elif (
                    op.kind is mir.Kind.ADD
                    and (first is not None or second is not None)
                    or op.kind is mir.Kind.SUB
                    and first is not None
                    and second is None
                ):
                    recurrence, offset = (first, right) if first is not None else (second, left)
                    if isinstance(offset, mir.Cell):
                        ref = offset.ref
                        if (
                            ref.addr is None
                            or ref.base is not None
                            and ref.base.id not in still
                            or ref.segment is not None
                            or ref.width != width
                            or not settled(ref)
                        ):
                            continue
                    elif not (
                        isinstance(offset, (mir.Const, mir.Held))
                        and offset.width == width
                        and (isinstance(offset, mir.Const) or offset.value.id in still)
                    ):
                        continue
                    base, scale, offsets = recurrence
                    offsets = (*offsets, (offset, -1 if op.kind is mir.Kind.SUB else 1))
                elif op.kind is mir.Kind.MUL:
                    if first is not None and isinstance(right, mir.Const) and right.width == width:
                        base, scale = first[0], first[1] * right.n
                        offsets = tuple((arg, coefficient * right.n) for arg, coefficient in first[2])
                    elif second is not None and isinstance(left, mir.Const) and left.width == width:
                        base, scale = second[0], second[1] * left.n
                        offsets = tuple((arg, coefficient * left.n) for arg, coefficient in second[2])
                    else:
                        continue
                elif (
                    op.kind is mir.Kind.SHL
                    and first is not None
                    and isinstance(right, mir.Const)
                    and 0 <= right.n < width * 8
                ):
                    base, scale = first[0], first[1] << right.n
                    offsets = tuple((arg, coefficient << right.n) for arg, coefficient in first[2])
                else:
                    continue
                scale = consts.masked(scale, width)
                forms[result.value.id] = base, scale, offsets
                out[id(op)] = Derived(op, base, mir.Const(scale, width), offsets)
                changed = True
    return list(out.values())


def _extended(body, loop, op, forms, facts):
    """An extension preserves a recurrence only where its narrow value cannot wrap."""
    if len(op.args) != 1 or len(op.results) != 1 or op.loads or op.stores or op.barrier or op.merges:
        return None
    source, result = op.args[0], op.results[0]
    if not isinstance(source, mir.Held) or not isinstance(result, mir.Held) or source.width >= result.width:
        return None
    form = forms.get(source.value.id)
    if form is None:
        return None
    counter, scale, offsets = form
    width = source.width
    if counter.start.width != width:
        return None
    raw_start = _constant(counter.start, facts, width)
    raw_step = _constant(counter.step, facts, width)
    last = _last_counter(body, loop, counter, facts, width)
    constants = tuple((_constant(arg, facts, width), coefficient) for arg, coefficient in offsets)
    if raw_start is None or raw_step is None or last is None or any(value is None for value, _ in constants):
        return None
    step = _as_signed(raw_step, width)
    if not step:
        return None
    if op.kind is mir.Kind.SIGN_EXTEND:
        start = _as_signed(raw_start, width)
    elif op.kind is mir.Kind.ZERO_EXTEND and last >= 0:
        start = raw_start
    else:
        return None
    distance = last - start
    if distance * step < 0 or distance % step:
        return None
    count = distance // step + 1
    if count <= 0:
        return None

    mask = (1 << (width * 8)) - 1
    sign = 1 << (width * 8 - 1)
    if op.kind is mir.Kind.SIGN_EXTEND:
        signed_scale = _as_signed(scale & mask, width)
        initial = start * signed_scale + sum(_as_signed(value, width) * coefficient for value, coefficient in constants)
        stride = step * signed_scale
        limits = -sign, sign
    else:
        initial = (raw_start * scale + sum(value * coefficient for value, coefficient in constants)) & mask
        raw_stride = (raw_step * scale) & mask
        # Half the modulus has two equally valid directions. Without another
        # semantic fact, choosing either would invent a wide recurrence.
        if raw_stride == sign and count > 1:
            return None
        stride = _as_signed(raw_stride, width)
        limits = 0, mask + 1
    final = initial + (count - 1) * stride
    if not all(limits[0] <= value < limits[1] for value in (initial, final)):
        return None
    widened = Affine(
        result.value.id,
        mir.Const(consts.masked(initial, result.width), result.width),
        mir.Const(consts.masked(stride, result.width), result.width),
        loop.header,
    )
    return widened, 1, ()


def _multiplier(op: "mir.Op", by: "mir.Arg") -> "mir.Arg":
    """What the counter is multiplied by, whatever the operation writes it as.

    A shift names its *amount*, not its multiplier: `i shl 1` multiplies by
    two. Passing the amount through emitted `imul r,1` -- a multiply by one,
    which is a copy and which select refuses in the three-operand form
    anyway.
    """
    if op.kind is not mir.Kind.SHL or not isinstance(by, mir.Const):
        return by
    return mir.Const(1 << by.n, max(by.width, 2))


def of(
    body: mir.MirBody, dgroup: frozenset[int] = frozenset(), bounds: dict | None = None
) -> list[tuple[loopy.Loop, dict[int, Affine], list[Derived]]]:
    """Every loop in this body, with its counters and what they derive."""
    out = []
    for loop in loopy.loops(list(body.blocks), body.entry):
        found = basics(body, loop)
        if found:
            out.append((loop, found, derived(body, loop, found, dgroup, bounds)))
    return out
