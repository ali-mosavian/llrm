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
address. Both inner and outer loops are eligible.

Where the target has scaled addressing, a value read only as a far cell's
address is not given a recurrence at all: `b + i*m` stays in the loop with
`b` computed once, and lowering makes it the cell's `[base+index*scale]`.
Every such address shares the counter, so the loop advances one register.
A scale above one needs the counter as a dword, which is exact only while
it cannot wrap and the address only while it is in bounds.

The pricing half is here because allocation cannot own it. A counter this
pass invents is live around the whole loop, so a loop given more of them
than the target has registers gets every one of them spilled, and a spilled
counter costs reload, add and store where recomputing the expression it
replaced costs two instructions and carries nothing. The allocator sees the
decision, not the choice. `_RESERVE` and `Where.registers` are the budget.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.model.mir import Op
from qbopt.analysis import consts
from qbopt.analysis import liveness
from qbopt.model.mir import MirBody
from qbopt.analysis import induction
from qbopt.model.passes import Where
from qbopt.analysis import loops as loopy
from qbopt.objectfile.module import Space
from qbopt.model.passes import AddressForm
from qbopt.model.passes import MIRTransform
from qbopt.model.passes import OperationCosts

_DEFAULT_COSTS = OperationCosts()


class Strength(MIRTransform):
    name = "strength"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        from qbopt.optimize import indvars
        from qbopt.optimize import ivshare
        from qbopt.optimize import exitsink
        from qbopt.optimize import loopexit
        from qbopt.optimize import transform

        body = reduced(
            body,
            self.where.dgroup,
            self.where.bounds,
            self.where.registers,
            self.where.index_scales,
            self.where.call_registers,
            self.where.costs,
            address_forms=self.where.address_forms,
        )
        body = exitsink.sunk(transform.dead(ivshare.shared(body)))
        body = loopexit.evaluated(body)
        body = indvars.simplified(body)
        return indvars.zeroed(body)


def reduced(
    body: MirBody,
    dgroup: frozenset[int] = frozenset(),
    bounds: dict | None = None,
    registers: int = 0,
    scales: frozenset[int] = frozenset(),
    call_registers: int = 0,
    costs: OperationCosts = _DEFAULT_COSTS,
    address_forms: tuple[AddressForm, ...] = (),
) -> MirBody:
    """`body` with every multiply of a counter by an invariant made an add."""
    from qbopt.optimize import transform as passes

    found = induction.of(body, dgroup, bounds)
    if not found:
        return body

    live = liveness.live(body) if registers else None
    at_of = {block.at: block for block in body.blocks}
    references: dict[int, int] = {}
    for block in body.blocks:
        for op in block.ops:
            for value in op.uses:
                references[value.id] = references.get(value.id, 0) + 1
    taken = max((one.variable for one in ssa.values(body)), default=0)
    first = taken + 1
    ahead: dict[int, list[Op]] = {}
    behind: dict[int, list[Op]] = {}
    replacements: dict[int, Op | tuple[Op, ...]] = {}
    # A carried pointer replaces an address expression with a copy from the
    # loop phi.  Its immediately-following memory use should name that phi
    # directly, rather than spend a copy merely to use it as a cell base.
    # Bindings are certified once all formula choices are known below: a
    # value used on another block or on a loop exit must retain its copy.
    pointer_bindings: list[tuple[Op, mir.Value, mir.Value]] = []
    wide: set[mir.Value] = set()
    facts = consts.known(body) if scales or address_forms else {}
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
                or one.pointer is not None
                or one.op.kind is mir.Kind.DIVMOD
                or one.offsets
                and one.op.kind is mir.Kind.SHL
                or any(isinstance(offset, mir.Cell) for offset, _ in one.offsets)
                or one.op.kind is mir.Kind.ADD
                and any(isinstance(offset, mir.Held) for offset, _ in one.offsets)
            )
        ]
        if scales:
            candidates = [one for one in candidates if not _indexed(body, one)]
        # Priced, which is the half of LLVM's LSR this did not have. A
        # derived counter is a value live around the whole loop, and where
        # the loop already drives a register file's worth the allocator's
        # only answer is to spill it: reload, add and store every iteration,
        # three instructions where recomputing the expression is two and
        # carries nothing. qbdemo's plasma nest had five of them and spilled
        # all five, sixteen instructions in a latch to advance five counters.
        #
        # Against the recurrences the loop drives, not against pressure.
        # Peak pressure inside a loop is high exactly where the multiply
        # chain still is, so pricing against it refuses the array-indexing
        # reductions this pass exists for -- harr 1.77x to 5.28x, matrix
        # 1.03x to 1.63x. Values live across the backedge is no better: a
        # promoted loop legitimately carries more than the register file,
        # and harr's loops report nine.
        leaves = _formula_set(candidates)
        widened: dict[int, list[tuple[Op, Op]] | None] = {}
        native_forms = tuple(form for form in address_forms if not form.secondary)
        if not native_forms:
            native_forms = (AddressForm(2, scales),)
        secondary_forms = tuple(form for form in address_forms if form.secondary)
        native = {
            id(one.op): indexed
            for one in leaves
            if (
                indexed := next(
                    (
                        found
                        for form in native_forms
                        if (found := _legal_form(body, loop, one, form, facts, widened)) is not None
                    ),
                    None,
                )
            )
            is not None
        }
        secondary = {
            id(one.op): indexed
            for one in leaves
            if id(one.op) not in native
            and (
                indexed := next(
                    (
                        found
                        for form in secondary_forms
                        if (found := _legal_form(body, loop, one, form, facts, widened)) is not None
                    ),
                    None,
                )
            )
            is not None
        }
        room = len(candidates)
        capacity = registers
        if call_registers and any(op.kind is mir.Kind.CALL for at in loop.body for op in at_of[at].ops):
            capacity = min(capacity, call_registers) if capacity else call_registers
        if capacity:
            # The old fixed reserve saw only recurrences.  A C loop may also
            # retain invariant address owners and several short-lived values;
            # adding a backedge-live formula when that semantic peak already
            # fills the target makes the new recurrence the spill.  Keep the
            # historical recurrence budget as an upper bound, but let actual
            # MIR liveness expose tighter loops.  Values which are still worth
            # carrying *while spilled* are admitted by `_formula_set` below.
            room = max(
                0,
                min(
                    capacity - _recurrences(body, loop) - _RESERVE,
                    capacity - liveness.pressure(body, live, loop.body),
                ),
            )
        secondary_indexes = _secondary_indexes(
            leaves,
            room,
            frozenset(native),
            secondary,
            references=references,
        )
        free = frozenset(native) | secondary_indexes
        candidates = _formula_set(candidates, room, free, costs=costs, references=references)
        indexes = {
            id(one.op): (native | secondary)[id(one.op)]
            for one in candidates
            if id(one.op) in native or id(one.op) in secondary_indexes
        }
        # Only a counter every address indexes. One pointer left beside it can
        # end the loop in its place, and the index then costs the register
        # the counter would have given back.
        stepped = {one.of.value for one in candidates if id(one.op) not in indexes}
        indexes = {
            id(one.op): indexes[id(one.op)]
            for one in candidates
            if id(one.op) in indexes and one.of.value not in stepped
        }
        for value in {
            one.of.value for one in candidates if id(one.op) in indexes and indexes[id(one.op)][1].index_width > 2
        }:
            for op, widened_op in widened[value] or ():
                replacements.setdefault(id(op), widened_op)
        for one in candidates:
            if id(one.op) not in indexes or id(one.op) in replacements:
                continue
            scale, form = indexes[id(one.op)]
            answer = _answer(body, one.op)
            counter = next(phi.result for phi in at_of[loop.header].phis if phi.result.id == one.of.value)
            taken += 1
            base = mir.Value(id=_next(body, taken), at=preheader, variable=taken, version=1)
            ahead.setdefault(preheader, []).extend(_starts(base, one, preheader, counted=False))
            taken += len(one.offsets) * 2
            address = dict(
                kind=mir.Kind.ADD,
                op=ir.Operation.BINARY,
                name="add",
                source_backed=False,
                defines=(answer,),
                loads=(),
                stores=(),
                merges={},
                symbol=False,
            )
            if form.index_width == 2:
                replacements[id(one.op)] = replace(
                    one.op,
                    uses=(base, counter),
                    args=(mir.Held(base, 2), mir.Held(counter, 2)),
                    results=(mir.Held(answer, 2),),
                    **address,
                )
                continue
            taken += 1
            extended = mir.Value(id=_next(body, taken), at=preheader, variable=taken, version=1)
            ahead[preheader].append(
                replace(
                    _made(mir.Kind.ZERO_EXTEND, "movzx", extended, (mir.Held(base, 2),), preheader, one.op),
                    results=(mir.Held(extended, form.index_width),),
                )
            )
            taken += 1
            product = mir.Value(id=_next(body, taken), at=one.op.at, variable=taken, version=1)
            shift = _made(
                mir.Kind.SHL,
                "shl",
                product,
                (mir.Held(counter, form.index_width), mir.Const(scale.bit_length() - 1, 1)),
                one.op.at,
                one.op,
            )
            replacements[id(one.op)] = (
                shift,
                replace(
                    one.op,
                    uses=(extended, product),
                    args=(mir.Held(extended, form.index_width), mir.Held(product, form.index_width)),
                    results=(mir.Held(answer, form.index_width),),
                    **address,
                ),
            )
            wide.add(answer)
        candidates = [one for one in candidates if id(one.op) not in indexes]
        # Every remaining formula is a value live around the loop. Formula
        # selection above has already collapsed complete sibling groups and
        # rejected cheap formulas when they exceed this budget.  A selected
        # formula may still exceed ``room`` deliberately: `_formula_set`
        # proved that updating and loading its spilled recurrence costs less
        # than recomputing it.  Do not apply the register-only budget again
        # here or those pressure-priced choices can never reach allocation.
        added = 0
        # One counter an expression. Reads of `t[j]` through two counters
        # stepping alike are one recurrence, and given one each, a round at a
        # time, mod_link_anims never reached a fixed point.
        shared: dict[tuple, mir.Value] = {}
        for one in candidates:
            # Once each. A multiply inside a nest is derived in every loop
            # that contains it, and reducing it twice would set up two
            # counters for one value.
            answer = _answer(body, one.op)
            if answer is None or id(one.op) in replacements:
                continue
            width = _width(one.op)
            key = (one.of.start, one.of.step, one.by, one.offsets, one.pointer, width)
            if key in shared:
                replacements[id(one.op)] = _copying(one.op, shared[key], answer, width)
                if one.pointer is not None:
                    pointer_bindings.append((one.op, answer, shared[key]))
                continue
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
            taken += _start_temporary_count(one)
            behind.setdefault(latches[0], []).append(
                _made(
                    mir.Kind.PTR_OFFSET if one.pointer is not None else mir.Kind.ADD,
                    "" if one.pointer is not None else "add",
                    step,
                    (mir.Held(start, width), stride),
                    latches[0],
                    one.op,
                )
            )
            replacements[id(one.op)] = _copying(one.op, start, answer, width)
            if one.pointer is not None:
                pointer_bindings.append((one.op, answer, start))
            shared[key] = start
            added += 1

    if not replacements:
        return body
    pointer_rebases = _local_pointer_rebases(body, pointer_bindings, replacements)
    changed = replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(
                    _rebased(ssa.substituted(op, pointer_rebases.get(id(op), {})), wide)
                    for op in _woven(block, ahead.get(block.at, []), behind.get(block.at, []), replacements)
                ),
            )
            for block in body.blocks
        ),
    )
    return ssa.constructed(changed, frozenset(range(first, taken + 1)))


def _formula_set(
    candidates: list[induction.Derived],
    room: int | None = None,
    free: set[int] | frozenset[int] = frozenset(),
    *,
    costs: OperationCosts = _DEFAULT_COSTS,
    references: dict[int, int] | None = None,
) -> list[induction.Derived]:
    """Choose whole recurrence formulas, collapsing sibling address forms.

    A shared product followed by several invariant base additions has two
    useful representations: carry every resulting address, or carry the one
    product and retain the cheap additions. Selecting only some addresses is
    the bad third representation -- both the shared product and some of its
    children remain live. Collapse a complete sibling group whenever its leaf
    formulas do not fit, and leave unrelated formulas for the ordinary budget
    below to rank in source order.

    ``free`` names leaves that lowering can express as indexed memory forms;
    they consume no recurrence and must not make their shared parent win.
    """
    made = {
        one.op.results[0].value: one for one in candidates if one.op.results and isinstance(one.op.results[0], mir.Held)
    }
    consumed = {
        arg.value for one in candidates for arg in one.op.args if isinstance(arg, mir.Held) and arg.value in made
    }
    selected = {
        id(one.op)
        for one in candidates
        if one.op.results and isinstance(one.op.results[0], mir.Held) and one.op.results[0].value not in consumed
    }
    if room is None:
        return [one for one in candidates if id(one.op) in selected]

    def slots() -> int:
        return len(selected.difference(free))

    references = references or {}
    while slots() > room:
        overflow = slots() - room
        choices = []
        for order, parent in enumerate(candidates):
            if not parent.op.results or not isinstance(parent.op.results[0], mir.Held):
                continue
            result = parent.op.results[0].value
            children = [
                child
                for child in candidates
                if id(child.op) in selected
                and child.of == parent.of
                and any(isinstance(arg, mir.Held) and arg.value == result for arg in child.op.args)
            ]
            # An indexed child is already the zero-recurrence formula. A
            # single child saves no pressure by replacing it with its parent.
            if len(children) < 2 or any(id(child.op) in free for child in children):
                continue
            gain = len(children) - (0 if id(parent.op) in selected else 1)
            if gain > 0:
                relief = min(gain, overflow)
                cheapest = sorted(
                    references.get(child.op.results[0].value.id, 1)
                    for child in children
                    if child.op.results and isinstance(child.op.results[0], mir.Held)
                )[:relief]
                # Best case for retaining the leaves: allocation spills the
                # least-used children. Each then needs a memory update at the
                # latch and a reload at every address use. The extra ADD is a
                # pressure-risk charge for exceeding the stated capacity.
                spill = sum(costs.memory_update + uses * costs.load + costs.add for uses in cheapest)
                leaf = len(children) * costs.add
                collapsed = costs.add + len(children) * costs.address
                benefit = spill - (collapsed - leaf)
                choices.append((benefit, gain, -order, parent, children))
        if not choices:
            break
        _benefit, _gain, _order, parent, children = max(choices, key=lambda choice: choice[:3])
        selected.difference_update(id(child.op) for child in children)
        selected.add(id(parent.op))

    # A complete sibling collapse cannot help a lone formula.  If it exceeds
    # the pressure budget, compare the work it removes with the memory traffic
    # of carrying it as a spilled recurrence.  This is the missing half of the
    # candidate-set decision: `i * 2` is one cheap shift when recomputed, while
    # a variable multiply can remain profitable even when its recurrence has
    # to be updated and consumed from memory.
    while slots() > room:
        overflow = [one for one in candidates if id(one.op) in selected and id(one.op) not in free]
        priced = []
        for order, one in enumerate(overflow):
            result = one.op.results[0].value if one.op.results and isinstance(one.op.results[0], mir.Held) else None
            uses = references.get(result.id, 1) if result is not None else 1
            spilled = costs.memory_update + uses * costs.load
            priced.append((_recompute_cost(one, costs) - spilled, -order, one))
        if not priced:
            break
        benefit, _order, loser = min(priced, key=lambda choice: choice[:2])
        if benefit > 0:
            break
        selected.remove(id(loser.op))

    return [one for one in candidates if id(one.op) in selected]


def _secondary_indexes(
    candidates: list[induction.Derived],
    room: int,
    native: frozenset[int],
    secondary: dict[int, tuple[int, AddressForm]],
    *,
    references: dict[int, int] | None = None,
) -> frozenset[int]:
    """Activate costed address forms before overflowing recurrence storage.

    Native indexed leaves consume no recurrence slot. Other leaves may occupy
    the available register budget. If those leaves overflow it, a legal
    secondary address is the next representation to try: it spends encoding
    bytes and a target-specific per-use cost, but it does not create the
    loop-carried value that allocation would otherwise spill. Only the
    remaining overflow reaches sibling collapse and spill/recompute pricing.
    """
    references = references or {}
    slots = [one for one in candidates if id(one.op) not in native]
    overflow = max(0, len(slots) - room)
    choices = []
    for order, one in enumerate(slots):
        indexed = secondary.get(id(one.op))
        if indexed is None:
            continue
        _scale, form = indexed
        result = one.op.results[0].value if one.op.results and isinstance(one.op.results[0], mir.Held) else None
        uses = references.get(result.id, 1) if result is not None else 1
        # Extension is loop setup; prefix cost is paid by each addressed use.
        # Extra bytes break equal execution-cost choices without pretending
        # that code size is processor latency.
        choices.append((form.extension_cost + uses * form.use_cost, form.extra_bytes * uses, order, id(one.op)))
    return frozenset(choice[3] for choice in sorted(choices)[:overflow])


def _recompute_cost(one: induction.Derived, costs: OperationCosts) -> int:
    """Target-neutral cost of rebuilding a complete affine formula.

    ``Derived.op`` is only the formula's leaf.  A composed candidate such as
    ``24*i - 128 + base`` therefore cannot be priced as that leaf's final
    addition: reducing it removes the scale and every invariant addition too.
    ``by`` and ``offsets`` are the canonical whole formula and remain valid
    after the producer chain itself has been folded away.
    """
    if one.op.kind in (mir.Kind.DIV, mir.Kind.REM, mir.Kind.DIVMOD):
        return costs.divide
    if isinstance(one.by, mir.Const):
        scale = one.by.n
        if scale in (0, 1):
            work = 0
        elif scale > 0 and scale & (scale - 1) == 0:
            work = costs.shift
        else:
            work = costs.multiply
    else:
        work = costs.multiply
    work += len(one.offsets) * costs.add
    if one.pointer is not None or one.op.kind is mir.Kind.PTR_OFFSET:
        work += costs.address
    return work


def _copying(op: Op, start: mir.Value, answer: mir.Value, width: int) -> Op:
    """`op` made a copy of the counter that replaces it."""
    return replace(
        op,
        kind=mir.Kind.COPY,
        name="",
        source_backed=False,
        defines=(answer,),
        uses=(start,),
        args=(mir.Held(start, width),),
        results=(mir.Held(answer, width),),
        loads=(),
        stores=(),
        merges={},
        symbol=False,
    )


# What a loop's body needs to compute with, beyond the recurrences it
# drives. Swept against the documented suite and qbdemo's loops: at 1 and 2
# the suite is untouched, at 3 harr loses 182 instructions and nested 8. Two
# is the knee, and it is the number of operands an expression has.
_RESERVE = 2


def _recurrences(body: MirBody, loop) -> int:
    """How many recurrences the loop drives already, this pass's own included.

    Counted per round rather than per call. A derived counter is a phi
    advanced by a constant, so the next round reads it as a recurrence like
    any other -- which is what makes the budget shrink as the pass spends
    it. Counting only this call's additions let five rounds add five
    counters to qbdemo's plasma nest, one each, every one of them spilled.
    """
    return len(induction.basics(body, loop))


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


def _starts(into: mir.Value, one: induction.Derived, preheader: int, counted: bool = True) -> list[Op]:
    """Initialize scale * start plus invariant offsets once, before the loop.

    Not `counted`, the offsets alone: the base an index is added to.
    """
    # The direct pointer form is ``base + i``.  A composed offset has the
    # same recurrence but needs its initial ``i * scale + invariant`` built
    # before adding it to the invariant pointer; falling back to the direct
    # form would silently drop that scale or offset.
    if one.pointer is not None and isinstance(one.by, mir.Const) and one.by.n == 1 and not one.offsets:
        return [_made(mir.Kind.PTR_OFFSET, "", into, (one.pointer, one.of.start), preheader, one.op)]
    if one.pointer is None and not one.offsets:
        return [_start(into, one, preheader)]
    width = _width(one.op)
    count = _start_temporary_count(one, counted)
    temporaries = iter(
        mir.Value(into.id + 2 + number, preheader, variable=into.variable + 1 + number, version=1)
        for number in range(count)
    )
    current = None
    operations = []
    if counted:
        current = next(temporaries)
        operations.append(_start(current, one, preheader))
    for index, (offset, coefficient) in enumerate(one.offsets):
        if coefficient != 1:
            product = next(temporaries)
            operations.append(
                _made(
                    mir.Kind.MUL,
                    "imul",
                    product,
                    (offset, mir.Const(coefficient & ((1 << (width * 8)) - 1), width)),
                    preheader,
                    one.op,
                )
            )
            offset = mir.Held(product, width)
        result = next(temporaries) if one.pointer is not None or index != len(one.offsets) - 1 else into
        if current is None:
            kind = mir.Kind.LOAD if isinstance(offset, mir.Cell) else mir.Kind.COPY
            operations.append(_made(kind, "mov", result, (offset,), preheader, one.op))
        else:
            operations.append(_made(mir.Kind.ADD, "add", result, (mir.Held(current, width), offset), preheader, one.op))
        current = result
    if one.pointer is not None:
        assert current is not None
        operations.append(
            _made(
                mir.Kind.PTR_OFFSET,
                "",
                into,
                (one.pointer, mir.Held(current, width)),
                preheader,
                one.op,
            )
        )
    return operations


def _start_temporary_count(one: induction.Derived, counted: bool = True) -> int:
    """Every temporary `_starts` needs for this complete affine formula.

    A pointer result cannot reuse ``into`` for its final offset sum: ``into``
    is the final `PTR_OFFSET`, not a numerical offset.  Count each product
    and intermediate sum directly rather than assuming two slots per
    invariant; a scaled invariant needs both, and the counter itself is a
    separate temporary when it is materialized here.
    """
    direct_pointer = one.pointer is not None and isinstance(one.by, mir.Const) and one.by.n == 1 and not one.offsets
    if direct_pointer or one.pointer is None and not one.offsets:
        return 0
    return int(counted) + sum(
        int(coefficient != 1) + int(one.pointer is not None or index != len(one.offsets) - 1)
        for index, (_offset, coefficient) in enumerate(one.offsets)
    )


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
    kept = [made for op in block.ops for made in _replaced(replacements.get(id(op), op))]
    if ahead or behind:
        cut = len(kept)
        while cut and kept[cut - 1].kind in (mir.Kind.JUMP, mir.Kind.BRANCH):
            cut -= 1
        if cut < len(kept):
            at = kept[cut].at
        elif kept:
            at = kept[-1].at
        else:
            at = block.at
        inserted = [replace(op, at=at, absorbed=()) for op in (*ahead, *behind)]
        kept = kept[:cut] + inserted + kept[cut:]
    return kept


def _local_pointer_rebases(
    body: MirBody,
    bindings: list[tuple[Op, mir.Value, mir.Value]],
    replacements: dict[int, "Op | tuple[Op, ...]"],
) -> dict[int, dict[int, mir.Value]]:
    """The same-block memory users that may name a new pointer phi directly.

    A derived pointer's original value may be visible through a join or on a
    path which did not execute its defining PTR_OFFSET.  Replacing it
    globally with the carried recurrence would then invent an address.  A
    direct operation use is safe when the definition dominates its block (or
    follows it in the same block): the new recurrence has produced exactly
    that address before the use runs.  Phi inputs retain the original copy;
    their individual incoming edges need a separate reconstruction rule.
    """
    where = {id(op): (block.at, index) for block in body.blocks for index, op in enumerate(block.ops)}
    dominators = loopy.dominators(body.blocks, body.entry)
    users: dict[int, list[Op]] = {}
    for block in body.blocks:
        for op in block.ops:
            for value in op.uses:
                users.setdefault(value.id, []).append(op)
    rebases: dict[int, dict[int, mir.Value]] = {}
    for source, answer, carried in bindings:
        source_at, source_index = where[id(source)]
        uses = users.get(answer.id, [])
        uses = [
            user
            for user in uses
            if id(user) not in replacements
            and (
                where[id(user)][0] != source_at
                and source_at in dominators.get(where[id(user)][0], frozenset())
                or where[id(user)][0] == source_at
                and where[id(user)][1] > source_index
            )
        ]
        if not uses:
            continue
        # A consumer with two independent carried-pointer bases must retain
        # both identities unless they agree.  That makes the substitution a
        # local value rename rather than an address-specific special case.
        if any(answer.id in rebases.get(id(user), {}) and rebases[id(user)][answer.id] != carried for user in uses):
            continue
        for user in uses:
            rebases.setdefault(id(user), {})[answer.id] = carried
    return rebases


def _replaced(one: "Op | tuple[Op, ...]") -> tuple[Op, ...]:
    return one if isinstance(one, tuple) else (one,)


def _legal_form(
    body: MirBody,
    loop,
    one: induction.Derived,
    form: AddressForm,
    facts: dict,
    widened: dict[int, list[tuple[Op, Op]] | None],
) -> tuple[int, AddressForm] | None:
    """This derived address in one form, including exact-width proof."""
    scale = _indexable(body, loop, one, form.scales, facts)
    if scale is None:
        return None
    if form.index_width > 2:
        widened.setdefault(one.of.value, _widened(body, loop, facts, one.of.value))
        if widened[one.of.value] is None:
            return None
    return scale, form


def _indexable(body: MirBody, loop, one: induction.Derived, scales: frozenset[int], facts: dict) -> "int | None":
    """The scale this address is its counter times, where the counter can index it.

    From zero by one, so the counter is the index. A scale above one needs
    the counter as a dword, and the address is then exact only in bounds.
    """
    if (
        one.pointer is not None
        or not one.offsets
        or _width(one.op) != 2
        or not isinstance(one.by, mir.Const)
        or one.by.n not in scales
        or induction._signed(one.of.start, facts, 2) != 0
        or induction._signed(one.of.step, facts, 2) != 1
    ):
        return None
    answer = _answer(body, one.op)
    refs = _addressed(body, answer) if answer is not None else None
    if not refs or any(ref.where is not Space.FAR or ref.base_width != 2 for ref in refs):
        return None
    if one.by.n > 1 and any(ref.allocation is None for ref in refs):
        return None
    return one.by.n


def _indexed(body: MirBody, one: induction.Derived) -> bool:
    """Whether this is already a cell's base plus its counter, as lowering folds it.

    Reducing it again would give the address back the recurrence the index
    replaced. A word is folded only unscaled, `[bx+si]`.
    """
    op = one.op
    answer = _answer(body, op)
    if op.kind is not mir.Kind.ADD or len(op.args) != 2 or answer is None:
        return False
    if not all(isinstance(arg, mir.Held) and arg.width == _width(op) for arg in op.args):
        return False
    made = {value: other for block in body.blocks for other in block.ops for value in other.defines}
    for arg in op.args:
        shift = made.get(arg.value)
        counted = arg.value.id == one.of.value or (
            _width(op) == 4
            and shift is not None
            and shift.kind is mir.Kind.SHL
            and isinstance(shift.args[0], mir.Held)
            and shift.args[0].value.id == one.of.value
        )
        if counted:
            return bool(_addressed(body, answer))
    return False


def _addressed(body: MirBody, value: mir.Value) -> "list[mir.MemRef] | None":
    """Every cell `value` is the address of, or None if anything else reads it."""
    fed = {phi.result for block in body.blocks for phi in block.phis if value in phi.incoming.values()}
    refs = []
    for block in body.blocks:
        for op in block.ops:
            if fed.intersection(op.uses):
                return None
            if value not in op.uses:
                continue
            cells = [one.ref for one in (*op.args, *op.results) if isinstance(one, mir.Cell)]
            found = [ref for ref in cells if ref.base == value]
            if (
                not found
                or any(isinstance(arg, mir.Held) and arg.value == value for arg in op.args)
                or any(ref.segment == value for ref in cells)
            ):
                return None
            refs.extend(found)
    return refs


def _widened(body: MirBody, loop, facts: dict, value: int | None = None) -> "list[tuple[Op, Op]] | None":
    """The ops setting and advancing this loop's counter, rewritten as dwords.

    Exact where the counter starts at a word constant no less than zero and
    stops before it could wrap, and nothing reads the flags its step sets.
    """
    at_of = {block.at: block for block in body.blocks}
    header = at_of[loop.header]
    made = {value: op for block in body.blocks for op in block.ops for value in op.defines}
    read = {value for block in body.blocks for op in block.ops for value in op.uses}
    phis = [phi for block in body.blocks for phi in block.phis]
    while grown := {value for phi in phis if phi.result in read for value in phi.incoming.values()} - read:
        read |= grown
    for affine in induction.basics(body, loop).values():
        if value is not None and affine.value != value:
            continue
        if induction._signed(affine.start, facts, 2) != 0 or induction._signed(affine.step, facts, 2) != 1:
            continue
        if induction._last_counter(body, loop, affine, facts, 2) is None:
            continue
        phi = next((phi for phi in header.phis if phi.result.id == affine.value), None)
        if phi is None or len(phi.incoming) != 2:
            continue
        ops = [made.get(value) for value in phi.incoming.values()]
        # A promoted slot the step also writes keeps its word: it reads the low half.
        if any(op is None or op.loads or op.stores or op.barrier or len(op.results) != 1 for op in ops):
            continue
        out = []
        for op in ops:
            if any(value.flags and value in read for value in op.defines):
                break
            match op.kind, op.args:
                case mir.Kind.COPY, (mir.Const(n=n, width=2),) if n >= 0:
                    args = (mir.Const(n, 4),)
                case mir.Kind.INCREMENT, (mir.Held(value=counter, width=2),) if counter == phi.result:
                    args = (mir.Held(counter, 4),)
                case mir.Kind.ADD, (mir.Held(value=counter, width=2), mir.Const(n=1)) if counter == phi.result:
                    args = (mir.Held(counter, 4), mir.Const(1, 4))
                case _:
                    break
            out.append((op, replace(op, args=args, results=(mir.Held(op.results[0].value, 4),))))
        else:
            return out
    return None


def _rebased(op: Op, wide: set[mir.Value]) -> Op:
    """`op` with every cell addressed through a widened value saying so."""
    if not wide or not wide.intersection(op.uses):
        return op

    def ref(one: mir.MemRef) -> mir.MemRef:
        return replace(one, base_width=4) if one.base in wide else one

    def arg(one):
        return replace(one, ref=ref(one.ref)) if isinstance(one, mir.Cell) else one

    return replace(
        op,
        args=tuple(map(arg, op.args)),
        results=tuple(map(arg, op.results)),
        loads=tuple(map(ref, op.loads)),
        stores=tuple(map(ref, op.stores)),
    )


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
        source_backed=False,
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
