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
from qbopt.analysis import peelsize
from qbopt.analysis import induction
from qbopt.model.passes import Where
from qbopt.analysis import floatfacts
from qbopt.model.passes import MIRTransform


class Unroll(MIRTransform):
    name = "unroll"

    def __init__(self, where: Where):
        self.where = where

    def transform(self, body):
        return expanded(body, self.where, self.where.calls)


def expanded(
    body: mir.MirBody,
    where: Where,
    calls: dict,
    *,
    skip: frozenset[int] = frozenset(),
) -> mir.MirBody:
    blocks = {block.at: block for block in body.blocks}
    predecessors = loops.predecessors(body.blocks)
    dgroup = where.dgroup
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
        (entry,) = outside
        (exit_at,) = exits
        if set(predecessors[exit_at]) != {header.at} or any(
            set(phi.incoming) != {header.at} for phi in blocks[exit_at].phis
        ):
            continue
        bridges = [blocks[at] for at in loop.body if at not in (header.at, latch.at)]
        if any(block.phis or len(block.succ) != 1 for block in bridges):
            continue
        path = []
        (at,) = set(header.succ) & loop.body
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
            else any(
                op.barrier
                or op.kind
                in (
                    mir.Kind.OPAQUE,
                    mir.Kind.CALL,
                    mir.Kind.RETURN,
                    mir.Kind.BRANCH,
                    mir.Kind.JUMP,
                    mir.Kind.SWITCH,
                )
                for op in repeated_ops
            )
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
            any(
                op.kind not in (mir.Kind.NOTHING, mir.Kind.STORE, mir.Kind.SUB, mir.Kind.BRANCH)
                or op.barrier
                or op.floating
                for op in header.ops
            )
            if floating_loop
            else any(
                op.barrier
                or op.stores
                or op.kind
                in (
                    mir.Kind.OPAQUE,
                    mir.Kind.CALL,
                    mir.Kind.RETURN,
                    mir.Kind.BRANCH,
                    mir.Kind.JUMP,
                    mir.Kind.SWITCH,
                    mir.Kind.ARG,
                    mir.Kind.RESULT,
                    mir.Kind.ESCAPE,
                )
                for op in header.ops[:-1]
            )
        )
        if invalid_header:
            continue
        count = induction.trip_count(body, loop, facts)
        if count is None:
            continue
        if count < 2 or not peelsize.admitted(body, loop, count, facts, where):
            continue
        if any(set(phi.incoming) != {entry, latch.at} for phi in header.phis):
            continue
        candidate = _expanded(body, loop, header, latch, bridge_ops, latch_ops, exit_at, entry, count)
        if count > 4 and floating_loop:
            exact = floatfacts.known(candidate, dgroup, calls)
            results = [
                arg.value
                for op in candidate.block(latch.at).ops
                if op.floating
                for arg in op.results
                if isinstance(arg, mir.Held) and arg.width == 10
            ]
            if not results or any(value not in exact for value in results):
                continue
        return candidate
    return body


def optimized(body: mir.MirBody, where: Where, *, watch=None) -> mir.MirBody:
    """Expand every exact loop `peelsize.admitted` prices as worth it, once each; the
    caller's fixed point settles the copies."""
    if not priced(body, where):
        return body
    while True:
        candidate = expanded(body, where, where.named)
        if candidate is body:
            return body
        if watch is not None:
            watch("unroll-accepted", candidate)
        body = candidate


def priced(body: mir.MirBody, where: Where) -> bool:
    """Whether the target prices every operation here, which a copy's cost needs."""
    return profit.static(body, where.costs) is not None


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
        results = tuple(
            replace(arg, value=defined.get(arg.value.id, arg.value)) if isinstance(arg, mir.Held) else arg
            for arg in read.results
        )
        return replace(
            read,
            defines=tuple(defined[value.id] for value in op.defines),
            results=results,
            absorbed=op.absorbed if owns else (),
            raised=None,
            symbol=op.symbol if owns else op.symbol is not False,
        )

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
    expanded.append(
        replace(
            header.ops[-1],
            at=anchor,
            kind=mir.Kind.JUMP,
            name="",
            args=(),
            results=(),
            uses=(),
            defines=(),
            loads=(),
            stores=(),
            merges={},
            source_backed=False,
            raised=((), ()),
            absorbed=(),
            target=exit_at,
            test=None,
            symbol=False,
        )
    )
    changed = []
    dominators = loops.dominators(body.blocks, body.entry)
    (first_iteration,) = set(header.succ) & loop.body
    for block in body.blocks:
        if block.at == header.at:
            ops = tuple(ssa.substituted(op, initial) for op in block.ops)
            # ``trip_count`` established a positive exact count before this
            # expansion was built.  The original zero-trip edge is therefore
            # impossible: retaining its guard leaves address calculations,
            # a branch and an exit phi around an otherwise constant body.
            # Keep the source occurrence as an unconditional control anchor;
            # the ordinary CFG cleanup will remove it when it falls through.
            guard = ops[-1]
            enter = replace(
                guard,
                kind=mir.Kind.JUMP,
                name="",
                args=(),
                results=(),
                uses=(),
                defines=(),
                loads=(),
                stores=(),
                merges={},
                source_backed=False,
                raised=((), ()),
                absorbed=guard.absorbed,
                target=first_iteration,
                test=None,
                symbol=False,
            )
            block = replace(block, phis=(), ops=(*ops[:-1], enter), succ=(first_iteration,))
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
            phis = tuple(
                replace(
                    phi,
                    incoming={
                        at: ssa.provider(value, swap) if exit_at in dominators.get(at, ()) else value
                        for at, value in phi.incoming.items()
                    },
                )
                for phi in block.phis
            )
            if block.at == exit_at:
                # A positive exact trip count removed the zero-trip edge.  The
                # expanded latch is the exit's sole remaining predecessor.
                phis = tuple(
                    replace(
                        phi,
                        incoming={latch.at: ssa.provider(phi.incoming[header.at], swap)},
                    )
                    for phi in block.phis
                )
            ops = (
                tuple(ssa.substituted(op, swap) for op in block.ops)
                if exit_at in dominators.get(block.at, ())
                else block.ops
            )
            block = replace(block, ops=ops, phis=phis)
        changed.append(block)
    pointer_values, pointer_seeds = ssa.cloned_pointer_metadata(body, iter(copies))
    integer_ranges = ssa.cloned_integer_ranges(body, iter(copies))
    return replace(
        body,
        blocks=tuple(changed),
        repetitions=(*body.repetitions, (latch.at, count)),
        pointer_values=pointer_values,
        pointer_seeds=pointer_seeds,
        integer_ranges=integer_ranges,
    )
