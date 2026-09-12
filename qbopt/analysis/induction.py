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
    wrote = [one for block in body.blocks if block.at in inside for op in block.ops for one in op.stores]

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
    test = branch.test
    if branch.target not in inside:
        test = {
            mir.Kind.LE: mir.Kind.GT,
            mir.Kind.LT: mir.Kind.GE,
            mir.Kind.GE: mir.Kind.LT,
            mir.Kind.GT: mir.Kind.LE,
            mir.Kind.EQ: mir.Kind.NE,
            mir.Kind.NE: mir.Kind.EQ,
        }.get(test)
    comparisons = [bound for op in header.ops[:-1] if (bound := _counter_bound(op, branch, counter, width)) is not None]
    if len(comparisons) != 1:
        return None
    start, step, bound = (_signed(arg, facts, width) for arg in (counter.start, counter.step, comparisons[0]))
    if start is None or step is None or bound is None or step == 0:
        return None
    if step > 0 and test in (mir.Kind.LE, mir.Kind.LT):
        limit = bound - (test is mir.Kind.LT)
        distance = limit - start
    elif step < 0 and test in (mir.Kind.GE, mir.Kind.GT):
        limit = bound + (test is mir.Kind.GT)
        distance = start - limit
    elif test is mir.Kind.NE and (bound - start) * step > 0 and (bound - start) % step == 0:
        distance = abs(bound - start) - abs(step)
    else:
        return None
    if distance < 0:
        return None
    last = start + (distance // abs(step)) * step
    sign = 1 << (width * 8 - 1)
    return last if -sign <= last + step < sign else None


def _counter_bound(op, branch, counter, width):
    if (
        len(op.args) != 2
        or op.loads
        or op.stores
        or op.barrier
        or op.merges
        or not isinstance(op.args[0], mir.Held)
        or op.args[0].value.id != counter.value
        or op.args[0].width != width
    ):
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
    return any(_last_counter(body, loop, counter, facts, 2) is not None for counter in basics(body, loop).values())


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
                    and op.kind is mir.Kind.SIGN_EXTEND
                    and op.results
                    and isinstance(op.results[0], mir.Held)
                    and op.results[0].value.id not in forms
                ):
                    extended = _extended(body, loop, op, forms, known)
                    if extended is not None:
                        forms[op.results[0].value.id] = extended
                        changed = True
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
    """Sign extension preserves a recurrence only across a proven non-wrapping range."""
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
    start = _signed(counter.start, facts, width)
    step = _signed(counter.step, facts, width)
    last = _last_counter(body, loop, counter, facts, width)
    constants = tuple((_signed(arg, facts, width), coefficient) for arg, coefficient in offsets)
    if start is None or step is None or last is None or any(value is None for value, _ in constants):
        return None
    offset = sum(value * coefficient for value, coefficient in constants)
    sign = 1 << (width * 8 - 1)
    if not all(-sign <= value * scale + offset < sign for value in (start, last)):
        return None
    widened = Affine(counter.value, mir.Const(start, result.width), mir.Const(step, result.width), loop.header)
    return widened, scale, tuple((mir.Const(value, result.width), coefficient) for value, coefficient in constants)


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
) -> list[tuple[object, dict[int, Affine], list[Derived]]]:
    """Every loop in this body, with its counters and what they derive."""
    out = []
    for loop in loopy.loops(list(body.blocks), body.entry):
        found = basics(body, loop)
        if found:
            out.append((loop, found, derived(body, loop, found, dgroup, bounds)))
    return out
