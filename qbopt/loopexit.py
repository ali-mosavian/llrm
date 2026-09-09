"""Evaluate affine exit values and delete finite, side-effect-free counted loops.

The integer equivalent of evaluating an AddRec at its backedge count: a
fixed increment sums to N * step; an affine increment also contributes
N(N-1)/2 times its stride. Results are modulo their own width. Only the
controlling recurrence must be proven not to wrap.
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
        if not counters:
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
        exits = _exit_terms(body, loop, counters, count)
        if exits is None:
            continue
        exit_at, = [at for at in header.succ if at not in loop.body]
        serial = max(value.id for value in ssa.values(body)) + 1
        variable = max(value.variable for value in ssa.values(body)) + 1
        calculations = []
        for phi in header.phis:
            terms = exits[phi.result.id]
            width = terms[0][0].width
            total = mir.Const(0, width)
            for arg, coefficient in terms:
                if isinstance(arg, mir.Const):
                    product = mir.Const(consts.masked(arg.n * coefficient, width), width)
                elif coefficient == 1:
                    product = arg
                else:
                    temporary = mir.Value(serial, header.at, variable=variable)
                    serial += 1
                    variable += 1
                    calculations.append(strength._made(mir.Kind.MUL, "", temporary,
                                        (arg, mir.Const(consts.masked(coefficient, width), width)), header.at, header.ops[0]))
                    product = mir.Held(temporary, width)
                temporary = mir.Value(serial, header.at, variable=variable)
                serial += 1
                variable += 1
                calculations.append(strength._made(mir.Kind.ADD, "", temporary,
                                    (total, product), header.at, header.ops[0]))
                total = mir.Held(temporary, width)
            calculations.append(strength._made(mir.Kind.COPY, "", phi.result,
                                (total,), header.at, header.ops[0]))
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


def _exit_terms(body, loop, counters, count):
    """Sum a linear increment over N iterations using N(N-1)/2, before modular reduction."""
    header = next(block for block in body.blocks if block.at == loop.header)
    made = {value: op for block in body.blocks if block.at in loop.body for op in block.ops for value in op.defines}
    still = induction.invariant(body, set(loop.body))
    headers = {phi.result for phi in header.phis}
    exits = {}
    for phi in header.phis:
        if phi.result.id in counters:
            counter = counters[phi.result.id]
            exits[phi.result.id] = [(counter.start, 1), (counter.step, count)]
            continue
        start, = [value for at, value in phi.incoming.items() if at not in loop.body]
        update, = [value for at, value in phi.incoming.items() if at in loop.body]
        op = made.get(update)
        if op is None or len(op.results) != 1 or not isinstance(op.results[0], mir.Held):
            return None
        width = op.results[0].width
        linear = _linear(mir.Held(update, width), made, headers, width, set(), {})
        if linear is None or linear.pop(mir.Held(phi.result, width), 0) != 1:
            return None
        terms = [(mir.Held(start, width), 1)]
        for arg, coefficient in linear.items():
            if isinstance(arg, mir.Const) or arg.value.id in still:
                terms.append((arg, coefficient * count))
            elif (counter := counters.get(arg.value.id)) is not None and counter.start.width == width:
                terms.extend(((counter.start, coefficient * count),
                              (counter.step, coefficient * count * (count - 1) // 2)))
            else:
                return None
        exits[phi.result.id] = terms
    return exits


def _linear(arg, made, headers, width, visiting, cached):
    if not isinstance(arg, (mir.Const, mir.Held)) or arg.width != width:
        return None
    if isinstance(arg, mir.Const) or arg.value in headers or arg.value not in made:
        return {arg: 1}
    if arg.value in visiting:
        return None
    if arg in cached:
        return cached[arg]
    op = made[arg.value]
    if op.results != (arg,) or op.loads or op.stores or op.merges:
        return None
    if op.kind is mir.Kind.COPY and len(op.args) == 1:
        parts = ((op.args[0], 1),)
    elif op.kind in (mir.Kind.ADD, mir.Kind.SUB) and len(op.args) == 2:
        parts = ((op.args[0], 1), (op.args[1], -1 if op.kind is mir.Kind.SUB else 1))
    elif op.kind in (mir.Kind.INCREMENT, mir.Kind.DECREMENT) and len(op.args) == 1:
        parts = ((op.args[0], 1), (mir.Const(1, width), -1 if op.kind is mir.Kind.DECREMENT else 1))
    else:
        return None
    result = {}
    for source, coefficient in parts:
        terms = _linear(source, made, headers, width, visiting | {arg.value}, cached)
        if terms is None:
            return None
        for term, factor in terms.items():
            result[term] = result.get(term, 0) + coefficient * factor
    cached[arg] = {term: factor for term, factor in result.items() if factor}
    return cached[arg]


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
