from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.analysis import loops
from qbopt.optimize import edges
from qbopt.model.passes import MIRTransform


class LoopSimplify(MIRTransform):
    name = "loopsimplify"

    def transform(self, body: mir.MirBody) -> mir.MirBody:
        return simplified(body)


def grouped(body: mir.MirBody, target: int, sources: frozenset[int]) -> mir.MirBody:
    destination = body.block(target)
    predecessors = loops.predecessors(body.blocks)
    if destination is None or not sources or not sources <= predecessors[target] or target == body.entry:
        return body
    parents = [block for block in body.blocks if block.at in sources]
    for parent in parents:
        if not parent.ops:
            return body
        last = parent.ops[-1]
        if last.kind is mir.Kind.BRANCH:
            if not edges.conditional(parent, target):
                return body
        elif last.kind is mir.Kind.JUMP:
            if parent.succ != (target,) or last.target != target:
                return body
        elif (
            parent.succ != (target,)
            or last.kind in {mir.Kind.CALL, mir.Kind.RETURN, mir.Kind.OPAQUE, mir.Kind.SWITCH, mir.Kind.ESCAPE}
            or last.barrier
        ):
            return body
    if any(set(phi.incoming) != predecessors[target] or phi.result.flags for phi in destination.phis):
        return body
    label = edges.fresh(body)
    serial = max((value.id for value in ssa.values(body)), default=-1) + 1
    versions = {}
    for value in ssa.values(body):
        versions[value.variable] = max(versions.get(value.variable, 0), value.version)
    bridge_phis = []
    target_phis = []
    for phi in destination.phis:
        incoming = {at: value for at, value in phi.incoming.items() if at in sources}
        values = set(incoming.values())
        if len(values) == 1:
            result = next(iter(values))
        else:
            variable = phi.result.variable
            versions[variable] = versions.get(variable, 0) + 1
            result = mir.Value(serial, label, variable=variable, version=versions[variable])
            serial += 1
            bridge_phis.append(mir.Phi(result, incoming))
        target_phis.append(
            replace(
                phi, incoming={**{at: value for at, value in phi.incoming.items() if at not in sources}, label: result}
            )
        )
    jump = mir.Op(
        label, ir.Operation.JUMP, "", (), (), kind=mir.Kind.JUMP, target=target, covers=(label, label), symbol=False
    )
    bridge = mir.MirBlock(label, tuple(bridge_phis), (jump,), (target,))
    changed = []
    for block in body.blocks:
        if block.at in sources:
            last = block.ops[-1]
            block = replace(
                block,
                succ=tuple(label if at == target else at for at in block.succ),
                ops=(*block.ops[:-1], replace(last, target=label) if last.target == target else last),
            )
        if block.at == target:
            block = replace(block, phis=tuple(target_phis))
        changed.append(block)
    return replace(body, blocks=(*changed, bridge))


def simplified(body: mir.MirBody) -> mir.MirBody:
    if loops.irreducible(body.blocks, body.entry):
        return body
    for original in loops.loops(body.blocks, body.entry):
        original = next(loop for loop in loops.loops(body.blocks, body.entry) if loop.header == original.header)
        candidate = body
        predecessors = loops.predecessors(candidate.blocks)
        outside = predecessors[original.header] - original.body
        if not outside:
            continue
        parent = candidate.block(next(iter(outside)))
        if parent is None:
            continue
        if len(outside) != 1 or parent.succ != (original.header,):
            candidate = grouped(candidate, original.header, outside)
            if candidate is body:
                continue
        if len(original.latches) != 1:
            changed = grouped(candidate, original.header, original.latches)
            if changed is candidate:
                continue
            candidate = changed
        current = next(
            loop for loop in loops.loops(candidate.blocks, candidate.entry) if loop.header == original.header
        )
        predecessors = loops.predecessors(candidate.blocks)
        exits = {
            at for block in candidate.blocks if block.at in current.body for at in block.succ if at not in current.body
        }
        for target in sorted(exits):
            sources = predecessors.get(target, frozenset()) & current.body
            if predecessors.get(target, frozenset()) - current.body:
                changed = grouped(candidate, target, sources)
                if changed is candidate:
                    break
                candidate = changed
        else:
            body = candidate
    return body
