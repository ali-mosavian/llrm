"""Bounded full unrolling of small exact-trip loops.

The MIR sequence is expanded in execution order. Reusing input provenance
does not give the emitter permission to sort it or discard cloned relocations.
Integer and floating loops use the same CFG/value mechanism; floating loops
have one additional gate because large expansions are useful only when exact
folding removes their cloned arithmetic.
"""

from dataclasses import replace

from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.analysis import loops
from qbopt.analysis import consts
from qbopt.optimize import profit
from qbopt.analysis import induction
from qbopt.model.passes import Where
from qbopt.analysis import floatfacts
from qbopt.model.passes import MIRTransform


class Unroll(MIRTransform):
    name = "unroll"

    def __init__(self, where: Where):
        self.where = where

    def transform(self, body):
        return expanded(body, self.where.dgroup, self.where.calls)


def expanded(
    body: mir.MirBody,
    dgroup: frozenset[int],
    calls: dict,
    *,
    skip: frozenset[int] = frozenset(),
) -> mir.MirBody:
    blocks = {block.at: block for block in body.blocks}
    predecessors = loops.predecessors(body.blocks)
    facts = consts.known(body, dgroup, calls)
    for loop in loops.loops(body.blocks, body.entry):
        if len(loop.latches) != 1:
            continue
        header = blocks[loop.header]
        latch = blocks[next(iter(loop.latches))]
        if latch.at in skip:
            continue
        outside = set(predecessors[header.at]) - loop.body
        exits = set(header.succ) - loop.body
        if len(outside) != 1 or len(exits) != 1 or latch.phis:
            continue
        entry, = outside
        exit_at, = exits
        if (set(predecessors[exit_at]) != {header.at}
            or any(set(phi.incoming) != {header.at} for phi in blocks[exit_at].phis)):
            continue
        bridges = [blocks[at] for at in loop.body if at not in (header.at, latch.at)]
        if any(block.phis or len(block.succ) != 1 for block in bridges):
            continue
        path = []
        at, = set(header.succ) & loop.body
        while at != latch.at and at not in path:
            path.append(at)
            at = blocks[at].succ[0]
        if at != latch.at or set(path) != {block.at for block in bridges}:
            continue
        ordered_bridges = tuple(blocks[at] for at in path)
        bridge_ops = tuple(
            op
            for block in ordered_bridges
            for op in (
                block.ops[:-1]
                if block.ops and block.ops[-1].kind is mir.Kind.JUMP and block.ops[-1].target == block.succ[0]
                else block.ops
            )
        )
        # The object frontend may express the backedge only in CFG while the
        # C frontend carries an explicit terminal JUMP.  They are the same
        # loop.  The jump is control, not one iteration's work, and the
        # expansion replaces it with one jump to the exit.
        latch_ops = latch.ops
        if latch_ops and latch_ops[-1].kind is mir.Kind.JUMP and latch_ops[-1].target == header.at:
            latch_ops = latch_ops[:-1]
        repeated_ops = (*bridge_ops, *latch_ops)
        floating_loop = any(op.floating for op in repeated_ops)
        invalid_latch = (
            any(op.barrier or op.kind in (mir.Kind.OPAQUE, mir.Kind.BRANCH, mir.Kind.JUMP) for op in repeated_ops)
            if floating_loop
            else any(op.barrier or op.kind in (
                mir.Kind.OPAQUE, mir.Kind.CALL, mir.Kind.RETURN,
                mir.Kind.BRANCH, mir.Kind.JUMP, mir.Kind.SWITCH,
            ) for op in repeated_ops)
        )
        if invalid_latch:
            continue
        if not header.ops or header.ops[-1].kind is not mir.Kind.BRANCH:
            continue
        # Keep the established floating-loop contract: compiler bookkeeping
        # may store its counter in the test block, but no floating operation
        # may be repeated there.  A new integer expansion accepts any pure
        # value computation and refuses observable memory/control effects.
        invalid_header = (
            any(op.kind not in (mir.Kind.NOTHING, mir.Kind.STORE, mir.Kind.SUB, mir.Kind.BRANCH)
                or op.barrier or op.floating for op in header.ops)
            if floating_loop
            else any(op.barrier or op.stores or op.kind in (
                mir.Kind.OPAQUE, mir.Kind.CALL, mir.Kind.RETURN, mir.Kind.BRANCH,
                mir.Kind.JUMP, mir.Kind.SWITCH, mir.Kind.ARG, mir.Kind.RESULT, mir.Kind.ESCAPE,
            ) for op in header.ops[:-1])
        )
        if invalid_header:
            continue
        count = induction.trip_count(body, loop, facts)
        if count is None:
            continue
        # This is only a compile-time/resource guard.  Whether the expanded
        # body is worth keeping is decided below with the selected CPU's
        # operation costs.  Keep enough room to evaluate a useful multi-block
        # loop while bounding the quadratic scalar analyses on cloned MIR.
        emitted = sum(op.kind is not mir.Kind.NOTHING for op in (*repeated_ops, *header.ops))
        if count < 2 or count * emitted > 512:
            continue
        if any(set(phi.incoming) != {entry, latch.at} for phi in header.phis):
            continue
        candidate = _expanded(body, loop, header, latch, bridge_ops, latch_ops, exit_at, entry, count)
        if count > 4 and floating_loop:
            exact = floatfacts.known(candidate, dgroup, calls)
            results = [arg.value for op in candidate.block(latch.at).ops if op.floating
                       for arg in op.results if isinstance(arg, mir.Held) and arg.width == 10]
            if not results or any(value not in exact for value in results):
                continue
        return candidate
    return body


def _size(body: mir.MirBody) -> int:
    return sum(len(block.phis) + sum(op.kind is not mir.Kind.NOTHING for op in block.ops) for block in body.blocks)


def _profitable(
    before: mir.MirBody,
    after: mir.MirBody,
    latch: int,
    count: int,
    where: Where,
) -> bool:
    """Whether exact dynamic savings pay for the optimized straight-line body."""
    return _rejection(before, after, latch, count, where) is None


def _rejection(
    before: mir.MirBody,
    after: mir.MirBody,
    latch: int,
    count: int,
    where: Where,
) -> str | None:
    """Why a structural candidate loses, or ``None`` when it wins.

    Keep the decision inspectable rather than returning an unexplained false:
    matmul's locally cheaper rejected peel was first mistaken for a later
    production pass because nothing recorded which gate had refused it.
    """
    if len(loops.loops(after.blocks, after.entry)) >= len(loops.loops(before.blocks, before.entry)):
        return "residual-loops"
    dynamic_before = profit.weighted(before, where.costs, {latch: count})
    dynamic_after = profit.weighted(after, where.costs)
    if dynamic_before is None or dynamic_after is None:
        return "unpriced"
    if dynamic_after >= dynamic_before:
        return "no-saving"
    # MIR cannot know final encoding bytes. Charge one register move per added
    # semantic operation: target-priced, bounded, and never an implicit free
    # expansion. Later selection still supplies the exact size measurement.
    growth = max(0, _size(after) - _size(before)) * where.costs.move
    return "growth" if dynamic_before - dynamic_after <= growth else None


def optimized(body: mir.MirBody, where: Where, *, optimize, watch=None) -> mir.MirBody:
    """Repeatedly expand one profitable exact loop and re-run scalar MIR."""
    rejected: set[int] = set()
    while True:
        candidate = expanded(body, where.dgroup, where.named, skip=frozenset(rejected))
        if candidate is body:
            return body
        additions = candidate.repetitions[len(body.repetitions) :]
        if len(additions) != 1:
            return body
        latch, count = additions[0]
        if latch in rejected:
            return body
        if watch is not None:
            watch("unroll-candidate", candidate)
        result = optimize(candidate)
        rejection = _rejection(body, result, latch, count, where)
        if rejection is not None:
            if watch is not None:
                watch(f"unroll-rejected-{rejection}", result)
            rejected.add(latch)
            continue
        body = result
        if watch is not None:
            watch("unroll-accepted", body)
        rejected.clear()


def _expanded(body, loop, header, latch, bridge_ops, latch_ops, exit_at, entry, count):
    values = tuple(ssa.values(body))
    next_id = max(value.id for value in values) + 1
    next_variable = max(value.variable for value in values) + 1
    swap = {phi.result.id: phi.incoming[entry] for phi in header.phis}
    initial = dict(swap)
    copies = []

    def clone(op, owns):
        nonlocal next_id, next_variable
        read = ssa.substituted(op, swap)
        defined = {}
        for value in op.defines:
            fresh = replace(value, id=next_id, variable=next_variable, version=1)
            next_id += 1
            next_variable += 1
            defined[value.id] = fresh
        copies.append(defined)
        swap.update(defined)
        results = tuple(replace(arg, value=defined.get(arg.value.id, arg.value))
                        if isinstance(arg, mir.Held) else arg for arg in read.results)
        return replace(read, defines=tuple(defined[value.id] for value in op.defines), results=results,
                       absorbed=op.absorbed if owns else (), raised=None,
                       symbol=op.symbol if owns else op.symbol is not False)

    expanded = []
    for iteration in range(count):
        if iteration:
            expanded.extend(clone(op, False) for op in header.ops[:-1])
            expanded.extend(clone(op, False) for op in bridge_ops)
        expanded.extend(clone(op, iteration == 0) for op in latch_ops)
        carried = {phi.result.id: ssa.provider(phi.incoming[latch.at], swap) for phi in header.phis}
        swap.update(carried)
    expanded.extend(clone(op, False) for op in header.ops[:-1])
    anchor = latch.ops[-1].at
    expanded.append(replace(header.ops[-1], at=anchor, kind=mir.Kind.JUMP, name="",
                            args=(), results=(), uses=(), defines=(), loads=(), stores=(),
                            merges={}, source_backed=False, raised=((), ()), absorbed=(),
                            target=exit_at, test=None, symbol=False))
    changed = []
    dominators = loops.dominators(body.blocks, body.entry)
    for block in body.blocks:
        if block.at == header.at:
            block = replace(block, phis=(), ops=tuple(ssa.substituted(op, initial) for op in block.ops))
        elif block.at == latch.at:
            block = replace(block, ops=tuple(expanded), succ=(exit_at,))
        elif block.at in loop.body:
            # The first iteration still reaches the original straight-line
            # bridge blocks.  Once the header phis are removed, those blocks
            # must read the entry values just as the cloned later iterations
            # read their carried values.  Leaving their phi operands behind
            # made C matmul lower three undefined address inputs.
            block = replace(block, ops=tuple(ssa.substituted(op, initial) for op in block.ops))
        elif block.at not in loop.body:
            # A phi reads on its incoming edge, not in the block containing
            # it.  An enclosing loop's header is not dominated by this exit,
            # but its backedge predecessor can be; substitute precisely those
            # edge uses. Ordinary operations still require block dominance.
            phis = tuple(replace(phi, incoming={
                at: ssa.provider(value, swap) if exit_at in dominators.get(at, ()) else value
                for at, value in phi.incoming.items()
            }) for phi in block.phis)
            if block.at == exit_at:
                # The entry test still owns its exit edge until branch folding.
                # The expanded latch reaches the exit with the final iteration.
                phis = tuple(replace(phi, incoming={
                    header.at: ssa.provider(phi.incoming[header.at], initial),
                    latch.at: ssa.provider(phi.incoming[header.at], swap),
                }) for phi in block.phis)
            ops = (tuple(ssa.substituted(op, swap) for op in block.ops)
                   if exit_at in dominators.get(block.at, ()) else block.ops)
            block = replace(block, ops=ops, phis=phis)
        changed.append(block)
    pointer_values, pointer_seeds = ssa.cloned_pointer_metadata(body, iter(copies))
    return replace(
        body,
        blocks=tuple(changed),
        repetitions=(*body.repetitions, (latch.at, count)),
        pointer_values=pointer_values,
        pointer_seeds=pointer_seeds,
    )
