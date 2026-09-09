"""Evaluate affine exit values and delete finite, side-effect-free counted loops.

The integer equivalent of evaluating an AddRec at its backedge count: the
header value after N updates is start + N * step, modulo its own width.
Only the controlling recurrence must be proven not to wrap.
"""

from dataclasses import replace

from qbopt import consts, induction, loops, mir, ssa


def evaluated(body: mir.MirBody) -> mir.MirBody:
    from qbopt import strength, transform

    facts = consts.known(body)
    blocks = {block.at: block for block in body.blocks}
    for loop in loops.loops(body.blocks, body.entry):
        if len(loop.body) != 2 or len(loop.latches) != 1:
            continue
        header = blocks[loop.header]
        latch = blocks[next(iter(loop.latches))]
        preheader = transform._preheader(body, loop)
        if preheader is None or blocks[preheader].succ != (header.at,) or latch.phis:
            continue
        counters = induction.basics(body, loop)
        if not counters or set(counters) != {phi.result.id for phi in header.phis}:
            continue
        counts = set()
        for counter in counters.values():
            width = counter.start.width
            last = induction._last_counter(body, loop, counter, facts, width)
            start = induction._signed(counter.start, facts, width)
            step = induction._signed(counter.step, facts, width)
            if last is not None and start is not None and step:
                counts.add((last - start) // step + 1)
        if len(counts) != 1 or not _disposable(body, loop, header, latch):
            continue
        count = counts.pop()
        exit_at, = [at for at in header.succ if at not in loop.body]
        serial = max(value.id for value in ssa.values(body)) + 1
        variable = max(value.variable for value in ssa.values(body)) + 1
        calculations = []
        for phi in header.phis:
            counter = counters[phi.result.id]
            width = counter.start.width
            if isinstance(counter.step, mir.Const):
                product = mir.Const(consts.masked(counter.step.n * count, width), width)
            else:
                temporary = mir.Value(serial, header.at, variable=variable)
                serial += 1
                variable += 1
                calculations.append(strength._made(mir.Kind.MUL, "", temporary,
                                    (counter.step, mir.Const(count, width)), header.at, header.ops[0]))
                product = mir.Held(temporary, width)
            calculations.append(strength._made(mir.Kind.ADD, "", phi.result,
                                (counter.start, product), header.at, header.ops[0]))
        jump = replace(header.ops[-1], kind=mir.Kind.JUMP, uses=(), args=(),
                       results=(), target=exit_at, made=None, raised=((), ()), test=None)
        # Preserve all original byte ownership while replacing the header's work.
        replacement = replace(header, phis=(), succ=(exit_at,),
                              ops=tuple(calculations) + tuple(_cleared(op) for op in header.ops[:-1]) + (jump,))
        changed = replace(body, blocks=tuple(
            replacement if block.at == header.at else
            replace(block, succ=(), ops=tuple(_cleared(op) for op in block.ops)) if block.at == latch.at else block
            for block in body.blocks
        ))
        return transform._trivial_phis(transform._unreachable(changed))
    return body


def _cleared(op):
    return replace(op, kind=mir.Kind.NOTHING, name="", defines=(), uses=(),
                   loads=(), stores=(), args=(), results=(), merges={}, made=None,
                   raised=None, target=None, test=None, stack=None, symbol=False)


def _disposable(body, loop, header, latch) -> bool:
    allowed = {mir.Kind.NOTHING, mir.Kind.COPY, mir.Kind.ADD, mir.Kind.SUB,
               mir.Kind.INCREMENT, mir.Kind.DECREMENT}
    for block in (header, latch):
        for op in block.ops:
            if op.loads or op.stores or op.merges or op.stack is not None:
                return False
            if block is header and op is header.ops[-1]:
                continue  # The trip-count proof checked this branch.
            if op.kind not in allowed or any(not isinstance(arg, (mir.Held, mir.Const)) for arg in op.args):
                return False
    internal = {value for block in (header, latch) for op in block.ops for value in op.defines}
    for block in body.blocks:
        if block.at in loop.body:
            continue
        if any(value in internal for op in block.ops for value in op.uses):
            return False
        if any(value in internal for phi in block.phis for value in phi.incoming.values()):
            return False
    return True
