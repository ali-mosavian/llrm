from dataclasses import replace

from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.analysis import loops


def closed(body: mir.MirBody, loop: loops.Loop) -> mir.MirBody:
    predecessors = loops.predecessors(body.blocks)
    dominators = loops.dominators(body.blocks, body.entry)
    exits = {at for block in body.blocks if block.at in loop.body for at in block.succ if at not in loop.body}
    if any(not predecessors.get(at) or predecessors[at] - loop.body for at in exits):
        return body
    definitions = {
        value: block.at
        for block in body.blocks
        if block.at in loop.body
        for value in (*(phi.result for phi in block.phis), *(value for op in block.ops for value in op.defines))
        if not value.flags
    }
    sites: dict[mir.Value, set[int]] = {}
    for block in body.blocks:
        if block.at in loop.body:
            continue
        for op in block.ops:
            for value in op.uses:
                if value in definitions:
                    sites.setdefault(value, set()).add(block.at)
        for phi in block.phis:
            for parent, value in phi.incoming.items():
                if parent not in loop.body and value in definitions:
                    sites.setdefault(value, set()).add(parent)
    for value in sorted(sites, key=lambda value: value.id):
        available = {
            at for at in exits if all(definitions[value] in dominators.get(parent, ()) for parent in predecessors[at])
        }
        needed = set()
        pending = list(sites[value])
        while pending:
            at = pending.pop()
            if at in needed:
                continue
            if at in loop.body or at == body.entry or not predecessors.get(at):
                break
            needed.add(at)
            if at not in available:
                pending.extend(predecessors[at])
        else:
            body = _merged(body, value, needed, available & needed, predecessors)
    return body


def _merged(
    body: mir.MirBody, value: mir.Value, needed: set[int], exits: set[int], predecessors: dict[int, frozenset[int]]
) -> mir.MirBody:
    reaching = {at: ({at} if at in exits else set()) for at in needed}
    while True:
        changed = {
            at: reaching[at] if at in exits else set().union(*(reaching[parent] for parent in predecessors[at]))
            for at in needed
        }
        if changed == reaching:
            break
        reaching = changed
    if any(not sources for sources in reaching.values()):
        return body
    values = tuple(ssa.values(body))
    serial = max((one.id for one in values), default=-1) + 1
    version = max((one.version for one in values if one.variable == value.variable), default=0) + 1
    joins = exits | {at for at in needed if len(reaching[at]) > 1 and len(predecessors[at]) > 1}
    replacements = {
        at: mir.Value(serial + offset, at, variable=value.variable, version=version + offset)
        for offset, at in enumerate(sorted(joins))
    }
    pending = needed - joins
    while pending:
        changed = set()
        for at in pending:
            if len(reaching[at]) == 1:
                replacements[at] = replacements[next(iter(reaching[at]))]
            else:
                (parent,) = predecessors[at]
                if parent not in replacements:
                    continue
                replacements[at] = replacements[parent]
            changed.add(at)
        if not changed:
            return body
        pending -= changed
    phis = {
        at: mir.Phi(
            replacements[at],
            {parent: value if at in exits else replacements[parent] for parent in sorted(predecessors[at])},
        )
        for at in joins
    }
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                phis=tuple(
                    replace(
                        phi,
                        incoming={
                            parent: replacements[parent] if incoming == value and parent in replacements else incoming
                            for parent, incoming in phi.incoming.items()
                        },
                    )
                    for phi in block.phis
                )
                + ((phis[block.at],) if block.at in phis else ()),
                ops=tuple(ssa.substituted(op, {value.id: replacements[block.at]}) for op in block.ops)
                if block.at in replacements
                else block.ops,
            )
            for block in body.blocks
        ),
    )
