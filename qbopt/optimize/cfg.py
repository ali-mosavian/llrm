"""Control-flow cleanup over values and semantic block identities."""

from dataclasses import replace

from qbopt.analysis import loops, ssa
from qbopt.model import ir, mir


def _empty(op):
    return (op.kind is mir.Kind.NOTHING and not op.name
            and not (op.defines or op.uses or op.args or op.results or op.loads or op.stores
                     or op.merges or op.barrier or op.floating or op.stack)
            and op.floating_origin is None)


def _owns_interval(blocks, start, end):
    """Layout cannot compact a body's chain across bytes owned by another body."""
    spans = sorted(span for block in blocks for op in block.ops
                   for span in (*((op.covers,) if op.covers is not None else ()), *op.extra_covers)
                   if span[0] < span[1])
    covered = start
    for low, high in spans:
        if high <= covered:
            continue
        if low > covered:
            break
        covered = high
        if covered >= end:
            return True
    return False


def merged(body: mir.MirBody) -> mir.MirBody:
    """Merge forward single-entry chains, retaining every original byte owner.

    Unreachable ownership-only blocks between the endpoints move with them.
    Other intervening blocks and unowned gaps retain their placement. Floating sequence and
    unrolling provenance still require their original block boundaries.
    """
    from qbopt.optimize.transform import _empty_operation

    repeated = dict(body.repetitions)
    while True:
        ordered = sorted(body.blocks, key=lambda block: block.at)
        predecessors = loops.predecessors(ordered)
        positions = {block.at: index for index, block in enumerate(ordered)}
        for index, first in enumerate(ordered):
            if len(first.succ) != 1 or first.at in repeated:
                continue
            target = first.succ[0]
            if target == body.entry or positions.get(target, -1) <= index or target in repeated:
                continue
            second = ordered[positions[target]]
            if set(predecessors[target]) != {first.at}:
                continue
            between = ordered[index + 1:positions[target]]
            inserted = all(op.covers is not None and op.covers[0] == op.covers[1]
                           and all(low == high for low, high in op.extra_covers)
                           for block in (first, *between, second) for op in block.ops)
            if not inserted and not _owns_interval(body.blocks, first.at, target):
                continue
            if any(block.at == body.entry or block.at in repeated or block.phis or block.succ or predecessors[block.at]
                   or any(not _empty(op) for op in block.ops) for block in between):
                continue
            if any(op.floating_origin is not None for block in (first, second) for op in block.ops):
                continue
            if any(set(phi.incoming) != {first.at} or phi.result in phi.incoming.values()
                   for phi in second.phis):
                continue
            ops = first.ops
            if ops and (ops[-1].barrier or ops[-1].kind is mir.Kind.OPAQUE):
                continue
            if ops and ops[-1].kind in {mir.Kind.BRANCH, mir.Kind.JUMP}:
                last = ops[-1]
                if last.kind is not mir.Kind.JUMP or last.target != target:
                    continue
                ops = (*ops[:-1], replace(_empty_operation(last), target=None, test=None))
            swaps = {phi.result.id: phi.incoming[first.at] for phi in second.phis}
            if any(value.id in swaps for value in swaps.values()):
                continue
            marker = mir.Op(target, ir.Operation.NOTHING, "", (), (), kind=mir.Kind.NOTHING,
                            covers=(target, target))
            moved = (*ops, *(op for block in between for op in block.ops), marker, *second.ops)
            combined = replace(first, ops=moved, succ=second.succ)
            removed = {target, *(block.at for block in between)}
            blocks = []
            for block in body.blocks:
                if block.at in removed:
                    continue
                if block.at == first.at:
                    block = combined
                phis = tuple(mir.Phi(phi.result, {
                    first.at if at == target else at: swaps.get(value.id, value)
                    for at, value in phi.incoming.items()
                }) for phi in block.phis)
                blocks.append(replace(block, phis=phis, ops=tuple(ssa.substituted(op, swaps) for op in block.ops)))
            body = replace(body, blocks=tuple(blocks))
            break
        else:
            return body
