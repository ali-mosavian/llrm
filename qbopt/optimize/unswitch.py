"""Specialize a loop around a pure invariant condition, entirely in MIR."""

from dataclasses import replace

from qbopt.analysis import loops, ssa
from qbopt.model import mir
from qbopt.model.passes import OperationCosts
from qbopt.optimize import edges, lcssa, loopclone, profit


def optimized(
    body: mir.MirBody,
    dgroup,
    calls,
    *,
    registers: int | None = None,
    call_registers: int = 0,
    index_scales: frozenset[int] | None = None,
    costs: OperationCosts | None = None,
    watch=None,
) -> mir.MirBody:
    from qbopt.optimize import transform

    candidate = specialized(body)
    if candidate is body:
        return body
    stages = [("unswitch", candidate)]
    result = transform.applied(candidate, dgroup, calls, unswitch_=False,
                               registers=registers, call_registers=call_registers,
                               index_scales=index_scales, costs=costs,
                               watch=lambda name, state: stages.append((name, state)))
    def size(state):
        return sum(op.kind is not mir.Kind.NOTHING for block in state.blocks for op in block.ops)
    prices = costs or OperationCosts()
    before, after = profit.weighted(body, prices), profit.weighted(result, prices)
    if (len(loops.loops(result.blocks, result.entry)) >= len(loops.loops(body.blocks, body.entry))
        or size(result) > size(body)
        or before is None
        or after is None
        or after > before):
        return body
    if watch is not None:
        for name, state in stages:
            watch(name, state)
    return result


def specialized(body: mir.MirBody) -> mir.MirBody:
    from qbopt.optimize import transform

    closed = lcssa.closed(body)
    owners = {value: block.at for block in closed.blocks for value in
              (*[phi.result for phi in block.phis],
               *[value for op in block.ops for value in op.defines])}
    dominators = loops.dominators(closed.blocks, closed.entry)
    predecessors = loops.predecessors(closed.blocks)
    for loop in loops.loops(closed.blocks, closed.entry):
        outside = set(predecessors[loop.header]) - loop.body
        if len(outside) != 1 or sum(len(closed.block(at).ops) for at in loop.body) > 128:
            continue
        entry, = outside
        for block in closed.blocks:
            if block.at not in loop.body or block.at == loop.header or len(block.succ) != 2 or not block.ops:
                continue
            branch = block.ops[-1]
            matched = transform._comparison(block, branch)
            if matched is None:
                continue
            _, compare = matched
            if (branch.loads or branch.stores or branch.barrier or branch.floating or branch.stack
                or branch.defines or branch.results or mir.partial(branch)
                or compare.loads or compare.stores or compare.barrier or compare.floating or mir.partial(compare)
                or compare.stack
                or not all(isinstance(arg, (mir.Held, mir.Const)) for arg in compare.args)
                or any(owners.get(value) in loop.body or
                       (value in owners and owners[value] not in dominators[entry]) for value in compare.uses)):
                continue
            copied = loopclone.peeled(closed, loop, 1)
            if copied is None:
                continue
            candidate = _specialized(closed, copied, loop, entry, block, compare, branch)
            return transform._trivial_phis(transform._unreachable(candidate))
    return body


def _specialized(body, copied, loop, entry, selected, compare, branch):
    originals = [block for block in body.blocks if block.at in loop.body]
    old_labels = {block.at for block in body.blocks}
    duplicates = [block for block in copied.blocks if block.at not in old_labels]
    labels = {original.at: duplicate.at for original, duplicate in zip(originals, duplicates)}
    latch, = loop.latches
    cloned_header, cloned_latch = labels[loop.header], labels[latch]
    values = tuple(ssa.values(copied))
    next_id = max(value.id for value in values) + 1
    next_variable = max(value.variable for value in values) + 1
    definitions = {value.id: replace(value, id=next_id + index, variable=next_variable + index, version=1)
                   for index, value in enumerate(compare.defines)}
    parent = body.block(entry)
    last = parent.ops[-1] if parent.ops else None
    replaces_jump = last is not None and last.kind is mir.Kind.JUMP
    anchor = last.at if last is not None else entry
    guard = replace(compare, at=anchor, defines=tuple(definitions[value.id] for value in compare.defines),
                    results=tuple(replace(result, value=definitions[result.value.id]) for result in compare.results),
                    source_backed=False, raised=None, absorbed=(), id=None, symbol=False)
    dispatch = replace(ssa.substituted(branch, definitions), at=anchor, target=cloned_header,
                       source_backed=False, raised=None, name="", id=None, symbol=False,
                       absorbed=last.absorbed if replaces_jump else ())
    parent = replace(parent, ops=(*(parent.ops[:-1] if replaces_jump else parent.ops), guard, dispatch),
                     succ=(loop.header, cloned_header))
    changed = []
    for block in copied.blocks:
        if block.at == entry:
            block = parent
        elif block.at == loop.header:
            block = body.block(loop.header)
        elif block.at == cloned_header:
            phis = tuple(replace(phi, incoming={**phi.incoming,
                         cloned_latch: residual.incoming[cloned_latch]})
                         for phi, residual in zip(block.phis, copied.block(loop.header).phis))
            block = replace(block, phis=phis)
        if block.at == cloned_latch:
            block = replace(block, succ=(cloned_header,), ops=tuple(
                replace(op, target=cloned_header) if op.target == loop.header else op for op in block.ops))
        if block.at in (selected.at, labels[selected.at]):
            taken = block.ops[-1].target
            destination = taken if block.at == labels[selected.at] else next(at for at in block.succ if at != taken)
            jump = replace(block.ops[-1], kind=mir.Kind.JUMP, target=destination, test=None,
                           name="", uses=(), args=(), defines=(), results=(), raised=None)
            block = replace(block, ops=(*block.ops[:-1], jump), succ=(destination,))
        changed.append(block)
    result = replace(copied, blocks=tuple(changed))
    for header in (loop.header, cloned_header):
        result = edges.split(result, entry, header, edges.fresh(result), ())
    for version in (set(loop.body), set(labels.values())):
        exits = [(block.at, target) for block in result.blocks if block.at in version
                 for target in block.succ if target not in version]
        for source, target in exits:
            if edges.conditional(result.block(source), target):
                result = edges.split(result, source, target, edges.fresh(result), ())
    return result
