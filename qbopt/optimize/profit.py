"""Machine-neutral profitability shared by MIR transforms.

The target boundary translates instruction forms into :class:`OperationCosts`.
Everything here prices semantic work only: MIR kinds, memory effects and CFG
frequency.  A kind without a price makes the answer unknown rather than cheap.
"""

from qbopt.model import mir
from qbopt.analysis import loops
from qbopt.analysis import liveness
from qbopt.model.passes import OperationCosts

_ALU = frozenset(
    {
        mir.Kind.ADD,
        mir.Kind.SUB,
        mir.Kind.ADD_CARRY,
        mir.Kind.SUB_BORROW,
        mir.Kind.INCREMENT,
        mir.Kind.DECREMENT,
        mir.Kind.AND,
        mir.Kind.OR,
        mir.Kind.XOR,
        mir.Kind.NEG,
        mir.Kind.NOT,
        mir.Kind.LT,
        mir.Kind.LE,
        mir.Kind.GT,
        mir.Kind.GE,
        mir.Kind.EQ,
        mir.Kind.NE,
        mir.Kind.BELOW,
        mir.Kind.BELOW_EQ,
        mir.Kind.ABOVE,
        mir.Kind.ABOVE_EQ,
    }
)
_MOVES = frozenset(
    {
        mir.Kind.COPY,
        mir.Kind.CONVERT,
        mir.Kind.SIGN_EXTEND,
        mir.Kind.ZERO_EXTEND,
        mir.Kind.EXTRACT,
        mir.Kind.CONCAT,
        mir.Kind.JOIN,
        mir.Kind.ARG,
        mir.Kind.RESULT,
    }
)


def operation(one: mir.Op, costs: OperationCosts) -> int | None:
    """Target price for semantic work, or None when it cannot be priced."""
    if one.kind is mir.Kind.NOTHING:
        return 0
    if one.kind is mir.Kind.FLOAD:
        return costs.float_load
    if one.kind is mir.Kind.FSTORE:
        return costs.float_store
    folded_update = len(one.loads) == len(one.stores) == 1 and one.loads == one.stores
    memory = costs.memory_update if folded_update else len(one.loads) * costs.load + len(one.stores) * costs.store
    if one.kind in (mir.Kind.LOAD, mir.Kind.STORE):
        return memory
    if folded_update:
        return memory
    if one.kind in _ALU:
        work = costs.add
    elif one.kind in _MOVES:
        work = costs.move
    elif one.kind in (mir.Kind.MUL, mir.Kind.SMULHI):
        work = costs.multiply
    elif one.kind in (mir.Kind.DIV, mir.Kind.REM, mir.Kind.DIVMOD, mir.Kind.UDIVMOD):
        work = costs.divide
    # A fixed-point product is a widening multiply then a shift back; a
    # quotient, the shift first. Unpriced, one left nbody's whole body
    # unpriceable and every loop candidate was built only to be refused.
    elif one.kind is mir.Kind.FIXED_MUL:
        work = costs.multiply + costs.shift
    elif one.kind is mir.Kind.FIXED_DIV:
        work = costs.divide + costs.shift
    elif one.kind in (mir.Kind.SHL, mir.Kind.SHR, mir.Kind.SAR):
        work = costs.shift
    elif one.kind in (mir.Kind.ADDRESS, mir.Kind.PTR_OFFSET):
        work = costs.address
    elif one.kind in (mir.Kind.FADD, mir.Kind.FSUB, mir.Kind.FNEG, mir.Kind.FABS, mir.Kind.FCOMPARE):
        work = costs.float_add
    elif one.kind is mir.Kind.FMUL:
        work = costs.float_multiply
    elif one.kind in (mir.Kind.FDIV, mir.Kind.FSQRT):
        work = costs.float_divide
    elif one.kind is mir.Kind.FCHECK:
        work = costs.float_store
    elif one.kind is mir.Kind.CALL:
        work = costs.call
    elif one.kind in (mir.Kind.RETURN, mir.Kind.ESCAPE):
        work = costs.return_
    elif one.kind in (mir.Kind.BRANCH, mir.Kind.SWITCH, mir.Kind.JUMP):
        work = costs.branch
    else:
        return None
    return work + memory


def _block(block: mir.MirBlock, costs: OperationCosts) -> int | None:
    priced = tuple(operation(one, costs) for one in block.ops)
    if any(one is None for one in priced):
        return None
    return len(block.phis) * costs.move + sum(one for one in priced if one is not None)


def static(body: mir.MirBody, costs: OperationCosts) -> int | None:
    """Semantic work present once in the body, independent of frequency."""
    priced = tuple(_block(block, costs) for block in body.blocks)
    return None if any(one is None for one in priced) else sum(one for one in priced if one is not None)


def _frequencies(body: mir.MirBody, trips: dict[int, int] | None = None) -> dict[int, int] | None:
    """Profile-free block frequencies, or ``None`` for conflicting proofs."""
    frequency = {block.at: 1 for block in body.blocks}
    trips = trips or {}
    for loop in loops.loops(body.blocks, body.entry):
        exact = {trips[at] for at in loop.latches if at in trips}
        if len(exact) > 1:
            return None
        factor = next(iter(exact), 10)
        for at in loop.body:
            if at in frequency:
                frequency[at] *= factor
    return frequency


def weighted(body: mir.MirBody, costs: OperationCosts, trips: dict[int, int] | None = None) -> int | None:
    """Profile-free expected work, using exact or ten trips per loop level.

    ``trips`` keys a proven count by latch address.  This lets full unrolling
    compare against the trip count it proved instead of pretending every loop
    runs ten times; every other loop retains the conventional factor of ten.
    """
    frequency = _frequencies(body, trips)
    if frequency is None:
        return None
    total = 0
    for block in body.blocks:
        priced = _block(block, costs)
        if priced is None:
            return None
        total += frequency[block.at] * priced
    return total


def spill_risk(
    body: mir.MirBody,
    costs: OperationCosts,
    capacity: int,
    trips: dict[int, int] | None = None,
) -> int | None:
    """Whole-live-range traffic needed to fit MIR within ``capacity``.

    This models the integer capacity supplied by :class:`Where`; x87 values
    have a separate stack allocator and do not consume it.  It walks every
    program point, chooses the cheapest still-resident values needed to relieve that
    point, and retains those choices for the rest of the body.  Retention is
    essential: one chosen live range may relieve several overlapping peaks,
    while disjoint pressure waves necessarily choose and pay for different
    values.  Pricing definitions and uses makes the result the cost of those
    whole-range choices rather than a count of pressure points.

    Literal and fixed-address values use their cheaper reconstruction price,
    matching the allocator's existing rematerialization rather than charging
    them a fictitious frame slot.  Everything is expressed in MIR values and
    machine-neutral target costs.
    """
    if capacity <= 0:
        return 0
    frequency = _frequencies(body, trips)
    if frequency is None:
        return None
    definitions: dict[mir.Value, int] = {}
    uses: dict[mir.Value, int] = {}
    recipes: dict[mir.Value, list[mir.Op]] = {}
    floating: set[mir.Value] = set()
    for block in body.blocks:
        each = frequency[block.at]
        for phi in block.phis:
            definitions[phi.result] = definitions.get(phi.result, 0) + each
            for value in phi.incoming.values():
                uses[value] = uses.get(value, 0) + each
        for op in block.ops:
            floating.update(
                arg.value for arg in (*op.args, *op.results) if isinstance(arg, mir.Held) and arg.width == 10
            )
            for value in op.defines:
                definitions[value] = definitions.get(value, 0) + each
                recipes.setdefault(value, []).append(op)
            for value in op.uses:
                uses[value] = uses.get(value, 0) + each

    while True:
        before = len(floating)
        for block in body.blocks:
            for phi in block.phis:
                if phi.result in floating or any(value in floating for value in phi.incoming.values()):
                    floating.add(phi.result)
                    floating.update(phi.incoming.values())
        if len(floating) == before:
            break

    def reconstruction(value: mir.Value) -> int | None:
        found = recipes.get(value, ())
        if len(found) != 1:
            return None
        op = found[0]
        if op.loads or op.stores or op.barrier or len(op.results) != 1:
            return None
        if op.kind is mir.Kind.COPY and len(op.args) == 1 and isinstance(op.args[0], mir.Const):
            return costs.move
        if op.kind is mir.Kind.ADDRESS and len(op.args) == 1 and isinstance(op.args[0], (mir.FrameAddress, mir.Symbol)):
            return costs.address
        return None

    traffic = {}
    for value in definitions.keys() | uses.keys():
        slot = definitions.get(value, 0) * costs.store + uses.get(value, 0) * costs.load
        rematerialize = reconstruction(value)
        traffic[value] = slot if rematerialize is None else min(slot, uses.get(value, 0) * rematerialize)

    found = liveness.live(body)
    spilled: set[mir.Value] = set()
    risk = 0

    def account(alive: set[mir.Value]) -> None:
        nonlocal risk
        values = [value for value in alive if not value.flags and value not in floating and value not in spilled]
        excess = len(values) - capacity
        if excess > 0:
            selected = sorted(values, key=lambda value: (traffic.get(value, 0), value.id))[:excess]
            spilled.update(selected)
            risk += sum(traffic.get(value, 0) for value in selected)

    for block in body.blocks:
        alive = set(found.live_out[block.at])
        account(alive)
        for op in reversed(block.ops):
            alive.difference_update(op.defines)
            alive.update(op.uses)
            account(alive)
    return risk


def pressure_adjusted(
    body: mir.MirBody,
    costs: OperationCosts,
    capacity: int,
    trips: dict[int, int] | None = None,
) -> int | None:
    """Semantic work plus finite-capacity whole-range spill traffic."""
    work = weighted(body, costs, trips)
    pressure = spill_risk(body, costs, capacity, trips)
    return None if work is None or pressure is None else work + pressure
