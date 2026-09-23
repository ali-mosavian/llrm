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
class CountedLoop:
    """The one proof of how many trips a loop makes, shared by every pass.

        i = start; loop { [i test bound?] body; i += step; [i test bound?] }

    ``test`` continues the loop, counter first; ``step`` is a nonzero
    constant. A pre-tested loop tests the header value before each trip; a
    post-tested one tests after each trip, the stepped value when
    ``stepped``. ``width`` is the compare's: a counter read narrower is
    counted modulo that width.

    ``count`` is the exact trip count when constant. ``trips`` places it
    when symbolic, which needs a pre-tested unit step: the only proofs with
    no ``count``. ``first`` and ``last`` are the header's signed values on
    the first and last trip, given only when nothing up to the exit wraps.
    ``maximum`` bounds the trips when the count is unknown.
    """

    counter: Affine
    phi: mir.Phi
    compare: mir.Op
    branch: mir.Op
    start: mir.Held | mir.Const
    bound: mir.Held | mir.Const
    test: mir.Kind
    preheader: int | None
    latch: int
    entered: int
    exit: int
    maximum: int | None = None
    step: int = 1
    posttested: bool = False
    stepped: bool = False
    count: int | None = None
    first: int | None = None
    last: int | None = None

    @property
    def inclusive(self) -> bool:
        return self.test in _INCLUSIVE

    @property
    def width(self) -> int:
        return self.bound.width

    @property
    def span(self) -> tuple[int, int] | None:
        """The signed values the header's counter takes on a trip, lowest first."""
        if self.first is None or self.last is None:
            return None
        return min(self.first, self.last), max(self.first, self.last)


_ASCENDING = frozenset({mir.Kind.LT, mir.Kind.LE, mir.Kind.BELOW, mir.Kind.BELOW_EQ})
_DESCENDING = frozenset({mir.Kind.GT, mir.Kind.GE, mir.Kind.ABOVE, mir.Kind.ABOVE_EQ})
_INCLUSIVE = frozenset({mir.Kind.LE, mir.Kind.BELOW_EQ, mir.Kind.GE, mir.Kind.ABOVE_EQ})
_UNSIGNED = frozenset({mir.Kind.BELOW, mir.Kind.BELOW_EQ, mir.Kind.ABOVE, mir.Kind.ABOVE_EQ})
# The preheader test `bound SKIPPED start` under which no trip runs.
_SKIPPED = {test: mir.MIRRORED[mir.NEGATED[test]] for test in (*_ASCENDING, *_DESCENDING, mir.Kind.NE)}


Computed = Callable[[mir.Kind, tuple[mir.Arg, ...]], mir.Held | mir.Const]


def skipped(proof: CountedLoop) -> tuple[tuple[mir.Held | mir.Const, mir.Held | mir.Const], mir.Kind] | None:
    """The preheader comparison, and the test on it, under which the loop runs no trips."""
    if proof.posttested:
        return None
    return (proof.bound, proof.start), _SKIPPED[proof.test]


def trips(proof: CountedLoop, computed: Computed) -> mir.Held | mir.Const | None:
    """Trips on the entered path, exact modulo the compare's width, or None where not expressible.

    `computed(kind, args)` places one preheader operation and returns its
    result. `counted` proved the count finite.
    """
    width = proof.width
    if proof.posttested:
        return None
    if proof.count is not None:
        return mir.Const(proof.count, width) if proof.count < 1 << 8 * width else None
    ahead, behind = (proof.bound, proof.start) if proof.step > 0 else (proof.start, proof.bound)
    count = computed(mir.Kind.SUB, (ahead, behind))
    return computed(mir.Kind.ADD, (count, mir.Const(int(proof.inclusive), width)))


def exit_value(proof: CountedLoop, computed: Computed) -> mir.Held | mir.Const | None:
    """The header's counter as a pre-tested loop that ran a trip leaves: the first value failing its test."""
    width = proof.width
    if proof.posttested:
        return None
    if proof.test is mir.Kind.NE:
        return proof.bound
    if isinstance(proof.start, mir.Const) and proof.count is not None:
        return mir.Const(consts.masked(proof.start.n + proof.count * proof.step, width), width)
    past = mir.Const(consts.masked(proof.step * proof.inclusive, width), width)
    if isinstance(proof.bound, mir.Const):
        return mir.Const(consts.masked(proof.bound.n + past.n, width), width)
    return computed(mir.Kind.ADD, (proof.bound, past))


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


@dataclass(frozen=True, slots=True)
class _Control:
    """Where a single-latch loop with one exit tests whether to go round again."""

    block: int
    preheader: int | None
    entered: int
    exit: int
    posttested: bool


def _control(body: mir.MirBody, loop: loopy.Loop) -> _Control | None:
    """The block whose final branch is the loop's only exit: its header, or its latch."""
    blocks = {block.at: block for block in body.blocks}
    if len(loop.latches) != 1 or loop.header not in blocks:
        return None
    latch = blocks.get(next(iter(loop.latches)))
    header = blocks[loop.header]
    inside = set(loop.body)
    if latch is None:
        return None
    if latch.succ == (header.at,):
        control, entered = header, [at for at in header.succ if at in inside]
    elif header.at in latch.succ:
        control, entered = latch, [header.at]
    else:
        return None
    exits = [at for at in control.succ if at not in inside]
    if (
        len(control.succ) != 2
        or len(entered) != 1
        or len(exits) != 1
        or not control.ops
        or control.ops[-1].kind is not mir.Kind.BRANCH
        or control.ops[-1].target not in control.succ
        or any(
            not blocks[at].succ or any(to not in inside for to in blocks[at].succ) for at in inside if at != control.at
        )
    ):
        return None
    outside = [at for at in loopy.predecessors(body.blocks).get(header.at, ()) if at not in inside]
    preheader = outside[0] if len(outside) == 1 and blocks[outside[0]].succ == (header.at,) else None
    return _Control(control.at, preheader, entered[0], exits[0], control is latch)


def counted(
    body: mir.MirBody, loop: loopy.Loop, facts: dict | None = None, *, inbounds: bool = False
) -> tuple[CountedLoop, ...]:
    """Prove every counter that alone decides when a single-exit loop leaves.

    Constant start and bound give an exact ``count``, and so does an
    equality sentinel a constant distance from the start. Otherwise the
    proof is symbolic, and only for a pre-tested unit step whose loop is
    proved finite: an exclusive or ``!=`` test always is; an inclusive one
    runs forever where ``bound`` is the end of its type, so needs a
    ``maximum``. Only with `inbounds` is one taken from the loop's memory
    accesses: that reads `derived`, which asks this for counts.
    """
    facts = consts.known(body) if facts is None else facts
    blocks = {block.at: block for block in body.blocks}
    shape = _control(body, loop)
    if shape is None:
        return ()
    header, control = blocks[loop.header], blocks[shape.block]
    inside = set(loop.body)
    branch = control.ops[-1]
    continuing = branch.test if branch.target in inside else mir.NEGATED.get(branch.test)
    latch = next(iter(loop.latches))
    still = invariant(body, inside)
    made = {value.id: op for block in body.blocks for op in block.ops for value in op.defines}
    proven = []
    for counter in basics(body, loop).values():
        phi = next((one for one in header.phis if one.result.id == counter.value), None)
        if phi is None or latch not in phi.incoming or not isinstance(counter.start, (mir.Held, mir.Const)):
            continue
        tested = {phi.result.id: False, phi.incoming[latch].id: True} if shape.posttested else {phi.result.id: False}
        comparisons = [found for op in control.ops[:-1] if (found := _compared(op, branch, tested, made)) is not None]
        if len(comparisons) != 1 or continuing is None:
            continue
        compare, width, bound, mirrored, stepped = comparisons[0]
        test = mir.MIRRORED[continuing] if mirrored else continuing
        step = _signed(counter.step, facts, counter.start.width)
        if (
            not step
            or width > counter.start.width
            or not isinstance(bound, (mir.Held, mir.Const))
            or bound.width != width
        ):
            continue
        if isinstance(bound, mir.Held) and bound.value.id not in still:
            continue
        step = _as_signed(consts.masked(step, width), width)
        if not step or not (
            test is mir.Kind.NE or (test in _ASCENDING and step > 0) or (test in _DESCENDING and step < 0)
        ):
            continue
        start = mir.Held(counter.start.value, width) if isinstance(counter.start, mir.Held) else counter.start
        start = mir.Const(consts.masked(start.n, width), width) if isinstance(start, mir.Const) else start
        begin, limit = _constant(start, facts, width), _constant(bound, facts, width)
        difference = _difference(bound, start, begin, limit, made, facts, width)
        count = first = last = maximum = None
        if difference is not None and test is mir.Kind.NE:
            count = _equal_after(difference, step, width, shape.posttested, stepped)
        elif begin is not None and limit is not None:
            count = _ordered_after(begin, limit, step, test, width, shape.posttested, stepped)
        if count is not None:
            maximum = count
            if _signed(counter.step, facts, counter.start.width) == step:
                first, last = _signed_span(counter.start, facts, width, count, step)
        elif shape.posttested or abs(step) != 1:
            continue
        else:
            maximum = _unit_maximum(body, loop, start, bound, begin, limit, step, test, inbounds)
            if maximum is None and test in _INCLUSIVE:
                continue
        proven.append(
            CountedLoop(
                counter,
                phi,
                compare,
                branch,
                start if begin is None else mir.Const(begin, width),
                bound if limit is None else mir.Const(limit, width),
                test,
                shape.preheader,
                latch,
                shape.entered,
                shape.exit,
                maximum,
                step,
                shape.posttested,
                stepped,
                count,
                first,
                last,
            )
        )
    return tuple(proven)


def _compared(
    op: mir.Op, branch: mir.Op, tested: dict[int, bool], made: dict[int, mir.Op]
) -> tuple[mir.Op, int, mir.Arg, bool, bool] | None:
    """(op, width, bound, mirrored, stepped) where `op` sets `branch`'s flags from a tested counter value."""
    if len(op.args) != 2 or op.loads or op.stores or op.barrier:
        return None
    flags = [value for value in op.defines if value.flags]
    if len(flags) != 1 or flags[0] not in branch.uses:
        return None
    for index, arg in enumerate(op.args):
        if not isinstance(arg, mir.Held) or _copied(arg, made).value.id not in tested:
            continue
        stepped = tested[_copied(arg, made).value.id]
        if op.kind is mir.Kind.SUB and not op.results and len(op.defines) == 1:
            return op, arg.width, op.args[1 - index], index == 1, stepped
        if op.kind in (mir.Kind.AND, mir.Kind.OR) and op.args[0] == op.args[1]:
            return op, arg.width, mir.Const(0, arg.width), False, stepped
    return None


def _difference(
    bound: mir.Held | mir.Const,
    start: mir.Held | mir.Const,
    begin: int | None,
    limit: int | None,
    made: dict[int, mir.Op],
    facts: dict,
    width: int,
) -> int | None:
    """`bound - start` modulo the width, when constant: both one root plus a constant."""
    if begin is not None and limit is not None:
        return consts.masked(limit - begin, width)
    root, ahead = anchored(bound, made, width, facts)
    other, behind = anchored(start, made, width, facts)
    return consts.masked(ahead - behind, width) if root == other else None


def anchored(
    arg: mir.Held | mir.Const, made: dict[int, mir.Op], width: int, facts: dict | None = None
) -> tuple[mir.Value | None, int]:
    """`arg` as a root value plus a constant, through copies and constant adds; a number has no root.

    Two values with one root are a constant apart, which is how a loop from
    `x - 32` to `x` is counted and how two counters starting 4 apart share one.
    """
    offset = 0
    while isinstance(arg, mir.Held) and arg.width == width:
        known = _constant(arg, facts or {}, width)
        if known is not None:
            arg = mir.Const(known, width)
            break
        op = made.get(arg.value.id)
        if op is None or op.loads or op.stores or op.barrier or op.merges or op.results != (arg,):
            break
        if op.kind is mir.Kind.COPY and len(op.args) == 1 and isinstance(op.args[0], (mir.Held, mir.Const)):
            arg = op.args[0]
        elif op.kind is mir.Kind.ADD and len(op.args) == 2 and sum(isinstance(one, mir.Const) for one in op.args) == 1:
            constant, arg = sorted(op.args, key=lambda one: not isinstance(one, mir.Const))
            offset += constant.n
        else:
            break
    if isinstance(arg, mir.Const):
        return None, consts.masked(arg.n + offset, width)
    return arg.value, consts.masked(offset, width)


def _equal_after(difference: int, step: int, width: int, posttested: bool, stepped: bool) -> int | None:
    """Trips until `start + k*step`, tested as the loop is shaped, first equals `start + difference`."""
    modulus = 1 << 8 * width
    lead = int(posttested and stepped)
    divisor = gcd(step % modulus, modulus)
    remaining = (difference - lead * step) % modulus
    if remaining % divisor:
        return None  # never equal: the loop does not end
    period = modulus // divisor
    return int(posttested) + remaining // divisor * pow(step % modulus // divisor, -1, period) % period


def _ordered_after(
    begin: int, limit: int, step: int, test: mir.Kind, width: int, posttested: bool, stepped: bool
) -> int | None:
    """Trips of an ordered test with constant ends, or None where a tested value would wrap first."""
    unsigned = test in _UNSIGNED
    low, high = (0, (1 << 8 * width) - 1) if unsigned else (-(1 << 8 * width - 1), (1 << 8 * width - 1) - 1)
    first = (begin if unsigned else _as_signed(begin, width)) + int(posttested and stepped) * step
    bound = limit if unsigned else _as_signed(limit, width)
    if not low <= first <= high:
        return None
    if step > 0:
        edge = bound + (test in _INCLUSIVE)
        tested = max(0, -((first - edge) // step))
    else:
        edge = bound - (test in _INCLUSIVE)
        tested = max(0, -((edge - first) // -step))
    return int(posttested) + tested if low <= first + tested * step <= high else None


def _signed_span(start: mir.Arg, facts: dict, width: int, count: int, step: int) -> tuple[int | None, int | None]:
    """The first and last signed header values over `count` trips, where none up to the exit wraps."""
    begin = _signed(start, facts, start.width) if isinstance(start, (mir.Held, mir.Const)) else None
    if begin is None or count < 1 or _as_signed(consts.masked(begin, width), width) != begin:
        return None, None
    last = begin + (count - 1) * step
    sign = 1 << 8 * width - 1
    return (begin, last) if -sign <= last < sign and -sign <= last + step < sign else (None, None)


def _unit_maximum(
    body: mir.MirBody,
    loop: loopy.Loop,
    start: mir.Held | mir.Const,
    bound: mir.Held | mir.Const,
    begin: int | None,
    limit: int | None,
    step: int,
    test: mir.Kind,
    inbounds: bool,
) -> int | None:
    """Most trips of a symbolic unit-step loop, where proved; None for an inclusive test that may never end."""
    width = bound.width
    if test is mir.Kind.NE:
        return (1 << 8 * width) - 1
    unsigned, inclusive = test in _UNSIGNED, test in _INCLUSIVE
    low, high = (0, (1 << 8 * width) - 1) if unsigned else (-(1 << 8 * width - 1), (1 << 8 * width - 1) - 1)
    # Walked toward `bound`, as integers in the test's own signedness.
    begin = None if begin is None else begin if unsigned else _as_signed(begin, width)
    limit = None if limit is None else limit if unsigned else _as_signed(limit, width)
    if inclusive and limit == (high if step > 0 else low):
        return None
    ends = (_range(body, start, 0 if step > 0 else 1, high), _range(body, bound, 1 if step > 0 else 0, high))
    origin = begin if begin is not None else ends[0]
    target = limit if limit is not None else ends[1]
    if target is not None and limit is None and inclusive and target == (high if step > 0 else low):
        target = None
    if origin is not None and target is not None and low <= min(origin, target) and max(origin, target) <= high:
        return max(0, (target - origin) * step + inclusive)
    return _inbounds_trips(body, loop, next(iter(loop.latches))) if inbounds else None


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
    wide addresses at most 2**(8w) of them, so i*s + width <= 2**(8w). Only
    an access its frontend marked `inbounds` is promised that.
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
        if ref.inbounds and ref.base in step
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
    if proof.posttested or proof.preheader is None or proof.width != proof.counter.start.width:
        return None
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
    width = proof.width
    if (
        stepping is None
        or mir.Held(proof.phi.result, width) not in (mir.stepping(stepping) or ())
        or stepping.results != (mir.Held(update, width),)
        or stepping.loads
        or stepping.stores
        or stepping.barrier
        or stepping.merges
    ):
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


def controlling(body: mir.MirBody, loop: loopy.Loop, counter: Affine, facts: dict) -> CountedLoop | None:
    """The proof in which `counter` decides when `loop` leaves."""
    proofs = counted(body, loop, facts)
    return next((proof for proof in proofs if proof.counter.value == counter.value), None)


def domain(body: mir.MirBody, loop: loopy.Loop, affine: Affine, facts: dict) -> tuple[int, int] | None:
    """The finite inclusive signed domain ``affine`` takes on a trip."""
    proof = controlling(body, loop, affine, facts)
    return None if proof is None else proof.span


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
            span = domain(body, loop, counter, facts)
            if span is None or not all(-32768 <= value // denominator <= 32767 for value in span):
                continue
            quotient = Affine(
                counter.value,
                mir.Const(start // denominator, 2),
                mir.Const(step // denominator, 2),
                loop.header,
            )
            out.append(Derived(op, quotient, mir.Const(1, 2)))
    return out


def agreed_count(proofs: tuple[CountedLoop, ...]) -> int | None:
    """The one positive trip count every counter of a loop proves, if they prove one.

    A loop may carry an integer counter, a byte address and one or more
    derived counters at once.  They are evidence for the same trip count,
    not alternatives from which a transform may pick the convenient one.
    Refusing disagreement keeps cloning transforms independent of which
    recurrence happened to be visited first.
    """
    counts = {proof.count for proof in proofs if proof.count}
    return next(iter(counts)) if len(counts) == 1 else None


def trip_count(body: mir.MirBody, loop: loopy.Loop, facts: dict) -> int | None:
    """`agreed_count`, or the count remembered at this header when nothing proves one now."""
    proofs = counted(body, loop, facts)
    if not any(proof.count for proof in proofs):
        # A semantics-preserving loop transform may consume the syntactic
        # relationship which established this fact.  Nested-recurrence
        # rewind, for example, replaces ``start`` with an outer phi after it
        # has proved the inner loop's exact distance.  Retain that proof at
        # the same header so rotation and measurement do not fall back to a
        # guessed trip count.  A newly derived disagreement is still refused.
        return dict(body.loop_trip_counts).get(loop.header)
    return agreed_count(proofs)


def nonempty(body: mir.MirBody, loop) -> bool:
    """A counted loop whose first iteration and finite exit are proven."""
    return trip_count(body, loop, consts.known(body)) is not None


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
    proof = controlling(body, loop, counter, facts)
    count = proof.count if proof is not None and proof.width == width else None
    constants = tuple((_constant(arg, facts, width), coefficient) for arg, coefficient in offsets)
    if raw_start is None or raw_step is None or not count or any(value is None for value, _ in constants):
        return None
    step = _as_signed(raw_step, width)
    if not step or op.kind not in (mir.Kind.SIGN_EXTEND, mir.Kind.ZERO_EXTEND):
        return None
    start = _as_signed(raw_start, width) if op.kind is mir.Kind.SIGN_EXTEND else raw_start

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
