"""Join memory values, completing availability on non-speculative edges."""

from dataclasses import replace

from qbopt.analysis import loops, memoryssa, ssa
from qbopt.model import ir, mir
from qbopt.objectfile.module import Space


def _on_edge(ref, phis, predecessor):
    if not ref.pointer:
        return ref
    for phi in phis:
        if phi.result == ref.base:
            value = phi.incoming.get(predecessor)
            return replace(ref, base=value) if value is not None else None
    return ref


def _value(op):
    if op.barrier or op.floating is not None or op.stack is not None or op.merges:
        return None
    match op.kind, op.args, op.results:
        case mir.Kind.LOAD, (mir.Cell(ref=ref),), (mir.Held() as value,) if not op.stores and op.loads == (ref,):
            if op.defines != (value.value,):
                return None
        case mir.Kind.STORE, (mir.Held() as value,), (mir.Cell(ref=ref),) if not op.loads and op.stores == (ref,):
            if op.defines:
                return None
        case _:
            return None
    if value.width != ref.width or (ref.addr is None and not ref.pointer) or (ref.addr and ref.addr.space is Space.STACK):
        return None
    return ref, value


def _insertion(parent, join, index, translated, op, definitions, dominators, natural_loops):
    if (parent.succ != (join.at,) or join.at in dominators[parent.at]
        or any((parent.at in loop.body) != (join.at in loop.body) for loop in natural_loops)):
        return None
    for prior in join.ops[:index]:
        if (prior.kind not in {mir.Kind.NOTHING, mir.Kind.COPY}
            or prior.barrier or prior.floating is not None or prior.loads or prior.stores
            or prior.stack is not None or any(isinstance(arg, mir.Cell) for arg in prior.args)):
            return None
    incoming = {phi.result: phi.incoming.get(parent.at) for phi in join.phis}
    uses = tuple(incoming.get(value, value) for value in op.uses)
    address_uses = tuple(value for value in (translated.base, translated.segment) if isinstance(value, mir.Value))
    uses = tuple(dict.fromkeys((*uses, *address_uses)))
    cut = len(parent.ops)
    if cut and parent.ops[-1].kind is mir.Kind.JUMP:
        cut -= 1
    if any(prior.kind in {mir.Kind.BRANCH, mir.Kind.RETURN, mir.Kind.ESCAPE} for prior in parent.ops):
        return None
    for value in uses:
        if value is None or value.flags or value not in definitions:
            return None
        block, position = definitions[value]
        if block not in dominators[parent.at] or block == parent.at and position >= cut:
            return None
    return cut, uses


def reused(body: mir.MirBody, dgroup: frozenset[int] = frozenset(), *, insert: bool = False) -> mir.MirBody:
    predecessors = loops.predecessors(body.blocks)
    if not any(len(parents) > 1 for parents in predecessors.values()):
        return body
    graph = memoryssa.built(body)
    providers = [(site, value) for site, op in graph.operations.items() if (value := _value(op)) is not None]
    dominators = loops.dominators(body.blocks, body.entry)
    natural_loops = loops.loops(body.blocks, body.entry)
    fresh = max((value.id for value in ssa.values(body)), default=0) + 1
    by_at = {block.at: block for block in body.blocks}
    definitions = {value: (block.at, index) for block in body.blocks
                   for index, op in enumerate(block.ops) for value in op.defines}
    definitions.update({phi.result: (block.at, -1) for block in body.blocks for phi in block.phis})
    insertions = {}
    blocks = []
    for block in body.blocks:
        parents = predecessors[block.at]
        if block.at == body.entry or len(parents) < 2:
            blocks.append(block)
            continue
        phis, ops = list(block.phis), []
        for index, op in enumerate(block.ops):
            loaded = _value(op) if op.kind is mir.Kind.LOAD else None
            incoming = {}
            missing = {}
            if loaded is not None:
                ref, result = loaded
                site = memoryssa.Site(block.at, index)
                for parent in sorted(parents):
                    translated = _on_edge(ref, block.phis, parent)
                    if translated is None:
                        break
                    candidates = [(source, value) for source, (cell, value) in providers
                                  if source.block != block.at and source.block in dominators[parent]
                                  and block.at not in dominators[source.block]
                                  and value.width == result.width and graph.pointers.same_bytes(cell, translated)
                                  and all(source.block not in loop.body or block.at in loop.body for loop in natural_loops)
                                  and graph.available_on_edge(source, site, parent, ref, dgroup, edge_memory=translated)]
                    if not candidates:
                        placement = (_insertion(by_at[parent], block, index, translated, op, definitions,
                                                dominators, natural_loops) if insert else None)
                        if placement is None:
                            break
                        missing[parent] = (*placement, translated)
                        continue
                    _, value = max(candidates, key=lambda item: (len(dominators[item[0].block]), item[0].index))
                    incoming[parent] = value.value
            if not incoming or len(incoming) + len(missing) != len(parents):
                ops.append(op)
                continue
            for parent, (cut, uses, translated) in missing.items():
                predecessor = by_at[parent]
                at = predecessor.ops[min(cut, len(predecessor.ops) - 1)].at if predecessor.ops else parent
                value = mir.Value(fresh, at)
                fresh += 1
                load = mir.Op(at, ir.Operation.MOVE, "", (value,), uses, kind=mir.Kind.LOAD,
                              args=(mir.Cell(translated),), results=(mir.Held(value, result.width),),
                              loads=(translated,), covers=(at, at), symbol=False)
                insertions.setdefault(parent, []).append((cut, load))
                incoming[parent] = value
            value = mir.Value(fresh, block.at)
            fresh += 1
            phis.append(mir.Phi(value, incoming))
            ops.append(replace(op, kind=mir.Kind.COPY, args=(mir.Held(value, result.width),),
                               uses=(value,), loads=(), node=None, made=None, raised=None, symbol=False))
        blocks.append(replace(block, phis=tuple(phis), ops=tuple(ops)))
    for index, block in enumerate(blocks):
        ops = list(block.ops)
        for cut, load in sorted(insertions.get(block.at, ()), key=lambda item: item[0], reverse=True):
            ops.insert(cut, load)
        blocks[index] = replace(block, ops=tuple(ops))
    return replace(body, blocks=tuple(blocks))
