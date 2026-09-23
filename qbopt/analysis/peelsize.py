"""Whether a loop is worth copying out completely, decided before anything is cloned.

The decision is GCC's `try_unroll_loop_completely` (gcc/tree-ssa-loop-ivcanon.cc).
The size it is given is LLVM's `analyzeLoopUnrollCost`
(llvm/lib/Transforms/Scalar/LoopUnrollPass.cpp): each iteration is run over the
values it knows, what folds is free, and only the successors a folded branch
leaves are followed. That replaces GCC's `tree_estimate_loop_size` guess, which
credits a third of what is left as likely to fold. Unroll and peel ask this
alone; neither builds and optimizes a candidate to price it.
"""

from dataclasses import dataclass

from qbopt.model import mir
from qbopt.analysis import consts, induction
from qbopt.model.passes import Where

_OPAQUE = frozenset({mir.Kind.CALL, mir.Kind.OPAQUE, mir.Kind.ESCAPE, mir.Kind.ARG, mir.Kind.RESULT})

# GCC's `--param max-peel-branches`: undecided branches a copied sequence may hold.
MAX_PEEL_BRANCHES = 16
# LLVM's `-unroll-max-percent-threshold-boost`: how far saved work may raise the budget.
MAX_PERCENT_THRESHOLD_BOOST = 400


def admitted(body: mir.MirBody, loop, count: int, facts: dict, where: Where) -> bool:
    """Whether copying `loop` out `count` times pays: GCC's `try_unroll_loop_completely`.

    Past `max-completely-peel-times` iterations nothing is copied, however small the
    copy would settle: building it is the cost (deedlines' empty 16384-trip loops
    became 360K operations before they folded). A copy no larger than the loop always
    pays. Otherwise GCC refuses growth under -Os, with a call on the path (little is
    left to fold), past `max-peel-branches` undecided branches, and past
    `max-completely-peeled-insns` operations -- a budget raised, as LLVM's
    `shouldFullUnroll` raises it, by the share of the rolled work the copy no longer
    does (`getFullUnrollBoostingFactor`). A loop holding another is copied only when
    that shrinks it, as GCC does for outer loops.
    """
    limits = where.options
    if limits.max_unroll_iterations and count > limits.max_unroll_iterations:
        return False
    size, folded = _sizes(body, loop, facts)
    blocks = {block.at: block for block in body.blocks if block.at in loop.body}
    order = _ordered(blocks, loop.header)
    if order is None:
        return count * (size - folded) <= size
    budget = limits.max_unrolled_operations or 1 << 62
    limit = budget * MAX_PERCENT_THRESHOLD_BOOST // 100
    copy = _unrolled(blocks, order, loop, count, facts, where, max(limit, size))
    if copy is None:
        return False
    if copy.size <= size:
        return True
    return not (
        not limits.grows
        or copy.calls
        or copy.branches > MAX_PEEL_BRANCHES
        or copy.size > budget * _boost(copy) // 100
    )


@dataclass
class _Unrolled:
    """A complete copy's size, simulated rather than guessed."""

    size: int = 0  # operations no iteration folds, over every iteration
    branches: int = 0  # conditional branches no iteration decides, over every iteration
    calls: bool = False  # whether some iteration's path calls out
    rolled: int = 0  # operations the rolled loop executes: LLVM's `RolledDynamicCost`


def _boost(copy: _Unrolled) -> int:
    """LLVM's `getFullUnrollBoostingFactor`: the rolled work per unrolled operation, in percent, capped."""
    if not copy.size:
        return MAX_PERCENT_THRESHOLD_BOOST
    return min(100 * copy.rolled // copy.size, MAX_PERCENT_THRESHOLD_BOOST)


def _unrolled(blocks: dict, order: list[int], loop, count: int, facts: dict, where: Where, limit: int):
    """LLVM's `analyzeLoopUnrollCost`: run each of `count` iterations over the values it
    knows and the memory it has written, count what does not fold, and follow only the
    successors a folded branch leaves. None once more than `limit` operations remain,
    where LLVM bails out too."""
    from qbopt.optimize.transform import _comparison, _outcome, _switch_target

    (latch,) = loop.latches
    calls = where.named
    out = _Unrolled()
    cells: consts.Cells = {}
    previous = dict(facts)
    for iteration in range(count):
        values = dict(facts)
        carries: dict = {}
        came: dict[int, list[int]] = {loop.header: []}
        for at in order:
            if at not in came:
                continue
            block = blocks[at]
            for phi in block.phis:
                # The header's value comes from before the loop, then from the last iteration.
                if at == loop.header:
                    if iteration == 0:
                        source = next((value for pred, value in phi.incoming.items() if pred not in loop.body), None)
                    else:
                        source = phi.incoming.get(latch)
                    known = None if source is None else previous.get(source)
                elif len(came[at]) == 1 and came[at][0] in phi.incoming:
                    known = values.get(phi.incoming[came[at][0]])
                else:
                    known = None
                if known is None:
                    values.pop(phi.result, None)
                else:
                    values[phi.result] = known
            last = block.ops[-1] if block.ops else None
            found = _comparison(block, last) if last is not None else None
            compare = None if found is None else found[0]
            compared: dict = {}
            for index, op in enumerate(block.ops):
                if index == compare:
                    compared[(block.at, index)] = dict(cells)
                if op.kind is not mir.Kind.NOTHING:
                    out.rolled += 1
                # A compare is free when the branch reading it is decided, and counted there if not.
                if op.kind in (mir.Kind.NOTHING, mir.Kind.JUMP, mir.Kind.BRANCH, mir.Kind.SWITCH) or index == compare:
                    continue
                carry = consts._carry(op, values, cells)
                if carry is not None:
                    carries.update({value: carry for value in op.defines if value.flags})
                defined = consts._defined(op)
                result = (
                    consts._result(op, values, here=cells, carries=carries)
                    if defined is not None and _folds(op)
                    else None
                )
                if defined is not None and result is not None:
                    values[defined] = result
                else:
                    if defined is not None:
                        values.pop(defined, None)
                    out.size += 1
                    out.calls |= op.kind is mir.Kind.CALL
                cells = consts._kills(cells, op, values, where.dgroup, calls)
            if last is not None and last.kind is mir.Kind.BRANCH and len(block.succ) == 2:
                taken = _outcome(block, last, values, compared)
                decided = None if taken is None else [at for at in block.succ if (at == last.target) == taken]
            elif last is not None and last.kind is mir.Kind.SWITCH:
                target = _switch_target(last, values)
                decided = None if target is None else [target]
            else:
                decided = list(block.succ)
            if decided is None:
                out.branches += 1
                out.size += 1 + (compare is not None)
                decided = list(block.succ)
            for successor in decided:
                if successor != loop.header and successor in loop.body:
                    came.setdefault(successor, []).append(block.at)
            if out.size > limit:
                return None
        previous = values
    return out


def _ordered(blocks: dict, header: int) -> list[int] | None:
    """The loop's blocks, each after every block reaching it inside one iteration; None when the loop holds another."""

    def inner(at: int) -> bool:
        return at != header and at in blocks

    waiting = dict.fromkeys(blocks, 0)
    for block in blocks.values():
        for successor in block.succ:
            if inner(successor):
                waiting[successor] += 1
    ready = [header]
    order = []
    while ready:
        at = ready.pop()
        order.append(at)
        for successor in blocks[at].succ:
            if inner(successor):
                waiting[successor] -= 1
                if not waiting[successor]:
                    ready.append(successor)
    return order if len(order) == len(blocks) else None


def _folds(op: mir.Op) -> bool:
    """Whether an operation's result is a function of its inputs alone, memory it reads included."""
    return not (op.stores or op.barrier or op.kind in _OPAQUE or op.floating is not None)


def _sizes(body: mir.MirBody, loop, facts: dict) -> tuple[int, int]:
    """The loop's operations, and how many of them fold once the iteration is fixed."""
    inside = [block for block in body.blocks if block.at in loop.body]
    known = {
        one.value
        for one in induction.basics(body, loop).values()
        if _constant(one.start, facts) and _constant(one.step, facts)
    }
    known |= {value.id for value, fact in facts.items() if fact is not None}
    ops = [op for block in inside for op in block.ops if op.kind is not mir.Kind.NOTHING]
    folded: set[int] = set()
    changed = True
    while changed:
        changed = False
        for op in ops:
            if id(op) in folded or not _pure(op):
                continue
            if all(value.id in known for value in op.uses):
                folded.add(id(op))
                known.update(value.id for value in op.defines)
                changed = True
    phis = sum(len(block.phis) for block in inside)
    return len(ops) + phis, len(folded) + sum(phi.result.id in known for block in inside for phi in block.phis)


def _constant(arg: mir.Arg, facts: dict) -> bool:
    return isinstance(arg, mir.Const) or isinstance(arg, mir.Held) and facts.get(arg.value) is not None


def _pure(op: mir.Op) -> bool:
    return not (op.loads or op.stores or op.barrier or op.kind in _OPAQUE)
