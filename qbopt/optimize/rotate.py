"""Enter a loop proven to run at least once at its body, not at its test.

BC writes `FOR` as `jmp test; body: ...; test: cmp; jle body`. Where the
first test is proven to pass the entry jump goes straight to the body, and
the test is then reached only from the latch, which it can merge into.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.analysis import loops
from qbopt.analysis import induction


def entered(body: mir.MirBody) -> mir.MirBody:
    """Every proven loop entered at its body, each test merged into its latch.

    After the passes, not among them: a rotated loop is no longer the
    pretested shape the counted-loop analyses read, and peeling and
    unrolling it in a later round found no loop to work on.
    """
    from qbopt.optimize import cfg

    return cfg.merged(rotated(body))


def rotated(body: mir.MirBody) -> mir.MirBody:
    from qbopt.optimize import transform

    blocks = {block.at: block for block in body.blocks}
    predecessors = loops.predecessors(body.blocks)
    for loop in loops.loops(body.blocks, body.entry):
        if len(loop.latches) != 1:
            continue
        preheader = transform._preheader(body, loop)
        if preheader is None or blocks[preheader].succ != (loop.header,):
            continue
        header = blocks[loop.header]
        inside = [at for at in header.succ if at in loop.body]
        if len(header.succ) != 2 or len(inside) != 1 or inside[0] == header.at:
            continue
        first = blocks[inside[0]]
        if first.phis or set(predecessors.get(first.at, ())) != {header.at}:
            continue
        if not header.ops or header.ops[-1].kind is not mir.Kind.BRANCH:
            continue
        if not all(_tests(op) for op in header.ops[:-1]) or not induction.nonempty(body, loop):
            continue
        entry = blocks[preheader]
        ops = list(entry.ops)
        if ops and ops[-1].kind is mir.Kind.BRANCH:
            continue
        if ops and ops[-1].kind is mir.Kind.JUMP:
            ops[-1] = replace(ops[-1], target=first.at)
        else:
            at = ops[-1].at if ops else entry.at
            ops.append(
                mir.Op(at, ir.Operation.JUMP, "", (), (), kind=mir.Kind.JUMP, target=first.at, symbol=False)
            )
        return rotated(_entered(body, loop, preheader, header, first, ops))
    return body


def _entered(body, loop, preheader, header, first, ops) -> mir.MirBody:
    """`body` entering `loop` at `first`, its phis moved there by hand.

    Not re-derived by variable: two values of one variable a pass has
    already split are not one another's reaching definitions, and renaming
    `main`'s exit copy of a call's answer to the loop counter let decide
    fold the exit away.

    A header phi `p(preheader: a, latch: b)` becomes `q(preheader: a,
    header: b)` at `first`. The loop reads `q`; the test, now reached only
    from the latch, and everything after the loop read `b`.
    """
    latch = next(iter(loop.latches))
    serial = max(value.id for value in ssa.values(body)) + 1
    moved = {phi.result.id: replace(phi.result, id=serial + index, at=first.at) for index, phi in enumerate(header.phis)}

    def latest(value: mir.Value) -> mir.Value:
        # A latch value that is itself a header phi is that phi one pass on.
        return moved.get(value.id, value)

    ending = {phi.result.id: latest(phi.incoming[latch]) for phi in header.phis}
    inside = {at for at in loop.body if at != header.at}
    entry = tuple(
        mir.Phi(moved[phi.result.id], {preheader: phi.incoming[preheader], header.at: ending[phi.result.id]})
        for phi in header.phis
    )

    def rewired(block: mir.MirBlock) -> mir.MirBlock:
        swap = moved if block.at in inside else ending
        phis = tuple(
            replace(
                phi,
                incoming={
                    at: (moved if at in inside else ending).get(value.id, value) for at, value in phi.incoming.items()
                },
            )
            for phi in block.phis
        )
        changed = replace(block, phis=phis, ops=tuple(_swapped(op, swap) for op in block.ops))
        if block.at == preheader:
            return replace(changed, ops=tuple(ops), succ=(first.at,))
        if block.at == header.at:
            return replace(changed, phis=())
        if block.at == first.at:
            return replace(changed, phis=entry)
        return changed

    origin = dict(body.origin)
    pins = dict(body.pins)
    for phi in header.phis:
        for table in (origin, pins):
            if phi.result in table:
                table[moved[phi.result.id]] = table.pop(phi.result)
    return replace(body, blocks=tuple(rewired(block) for block in body.blocks), origin=origin, pins=pins)


def _swapped(op: mir.Op, swap: dict[int, mir.Value]) -> mir.Op:
    """One substitution step, never chained: a replacement is not itself replaced."""
    if not swap or not any(value.id in swap for value in op.uses):
        return op
    return ssa.substituted(op, {key: value for key, value in swap.items() if value.id not in swap or value.id == key})


def _tests(op: mir.Op) -> bool:
    """A comparison or nothing: the entry skips it, so it may compute nothing else.

    A floating check computes no value and still moves the x87 stack.
    """
    from qbopt.optimize import cfg

    if cfg._empty(op):
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
