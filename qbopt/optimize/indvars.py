"""Use an existing recurrence to control a loop instead of a redundant counter."""

from dataclasses import replace
from math import gcd

from qbopt.analysis import consts, induction, loops, ssa
from qbopt.model import mir


def simplified(body: mir.MirBody) -> mir.MirBody:
    from qbopt.optimize import loopexit, strength, transform

    facts = consts.known(body)
    blocks = {block.at: block for block in body.blocks}
    dominators = loops.dominators(body.blocks, body.entry)
    predecessors = loops.predecessors(body.blocks)
    for loop in loops.loops(body.blocks, body.entry):
        if len(loop.latches) != 1:
            continue
        preheader = transform._preheader(body, loop)
        if preheader is None or blocks[preheader].succ != (loop.header,):
            continue
        header = blocks[loop.header]
        counters = induction.basics(body, loop)
        for counter in counters.values():
            width = counter.start.width
            last = induction._last_counter(body, loop, counter, facts, width)
            start = induction._signed(counter.start, facts, width)
            step = induction._signed(counter.step, facts, width)
            if last is None or start is None or not step:
                continue
            count = (last - start) // step + 1
            phi = next(phi for phi in header.phis if phi.result.id == counter.value)
            update = phi.incoming[next(iter(loop.latches))]
            branch = header.ops[-1]
            compare = next(op for op in header.ops[:-1]
                           if induction._counter_bound(op, branch, counter, width) is not None)
            exit_at, = [at for at in header.succ if at not in loop.body]
            if set(predecessors.get(exit_at, ())) != {header.at} or not blocks[exit_at].ops:
                continue
            closed = {
                value: other
                for other in blocks[exit_at].phis
                if len(other.incoming) == 1
                for predecessor, value in other.incoming.items()
                if predecessor in loop.body
            }
            following = {at for at in blocks if exit_at in dominators.get(at, ())}
            if any(value in transform._leaving(body) for value in (phi.result, update)):
                continue
            if any(phi.result in op.uses and op is not compare and update not in op.defines
                   for block in body.blocks if block.at not in following for op in block.ops):
                continue
            if any(update in op.uses for block in body.blocks for op in block.ops):
                continue
            if any((phi.result in other.incoming.values() or update in other.incoming.values())
                   and other is not phi and other not in closed.values()
                   for block in body.blocks for other in block.phis):
                continue
            if any(set(compare.defines) & set(op.uses) and op is not branch
                   for block in body.blocks for op in block.ops):
                continue
            if any(set(compare.defines) & set(other.incoming.values())
                   for block in body.blocks for other in block.phis):
                continue
            for alternative in counters.values():
                stride = induction._signed(alternative.step, facts, width)
                if (alternative.value == counter.value or alternative.start.width != width or not stride
                    or count >= (1 << (8 * width)) // gcd(abs(stride), 1 << (8 * width))):
                    continue
                value = next(phi.result for phi in header.phis if phi.result.id == alternative.value)
                if not any(value in op.uses and not any(result.id == alternative.value for result in op.defines)
                           and op.kind not in (mir.Kind.ADD, mir.Kind.INCREMENT, mir.Kind.SUB)
                           for block in body.blocks if block.at in loop.body for op in block.ops):
                    continue
                serial = max(value.id for value in ssa.values(body)) + 1
                variable = max(value.variable for value in ssa.values(body)) + 1
                seed_at = blocks[preheader].ops[-1].at
                bound = mir.Value(serial, seed_at, variable=variable)
                final = mir.Value(serial + 1, exit_at, variable=variable + 1)
                seed = strength._made(mir.Kind.ADD, "", bound,
                    (alternative.start, mir.Const(consts.masked(stride * count, width), width)),
                    seed_at, blocks[preheader].ops[-1])
                finish = strength._made(mir.Kind.COPY, "", final,
                    (mir.Const(consts.masked(last + step, width), width),), exit_at, blocks[exit_at].ops[0])
                swap = {counter.value: final}
                removed = {
                    other.result
                    for value, other in closed.items()
                    if value in (phi.result, update)
                }
                swap.update({value.id: final for value in removed})
                changed = loopexit._substituted_exits(body, exit_at, following, [finish], swap)
                if removed:
                    changed = replace(
                        changed,
                        blocks=tuple(
                            replace(block, phis=tuple(other for other in block.phis if other.result not in removed))
                            for block in changed.blocks
                        ),
                    )
                out = []
                for block in changed.blocks:
                    ops = []
                    for op in block.ops:
                        if op is compare:
                            op = replace(op, args=(mir.Held(value, width), mir.Held(bound, width)),
                                         kind=mir.Kind.SUB, results=(),
                                         defines=tuple(value for value in op.defines if value.flags),
                                         uses=(value, bound), loads=(), node=None, made=None, raised=None)
                        elif op is branch:
                            op = replace(op, test=mir.Kind.NE if branch.target in loop.body else mir.Kind.EQ,
                                         name="", node=None, made=None, raised=((), ()))
                        ops.append(op)
                    if block.at == preheader:
                        ops.insert(len(ops) - 1, seed)
                    out.append(replace(block, ops=tuple(ops)))
                return replace(changed, blocks=tuple(out))
    return body
