"""Machine-neutral profitability shared by MIR transforms.

The target boundary translates instruction forms into :class:`OperationCosts`.
Everything here prices semantic work only: MIR kinds, memory effects and CFG
frequency.  A kind without a price makes the answer unknown rather than cheap.
"""

from qbopt.model import mir
from qbopt.analysis import loops
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
    memory = (
        costs.memory_update
        if folded_update
        else len(one.loads) * costs.load + len(one.stores) * costs.store
    )
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


def weighted(body: mir.MirBody, costs: OperationCosts, trips: dict[int, int] | None = None) -> int | None:
    """Profile-free expected work, using exact or ten trips per loop level.

    ``trips`` keys a proven count by latch address.  This lets full unrolling
    compare against the trip count it proved instead of pretending every loop
    runs ten times; every other loop retains the conventional factor of ten.
    """
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
    total = 0
    for block in body.blocks:
        priced = _block(block, costs)
        if priced is None:
            return None
        total += frequency[block.at] * priced
    return total
