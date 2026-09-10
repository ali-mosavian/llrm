"""Join independently available memory values without inserting new reads."""

from dataclasses import replace

from qbopt.analysis import loops, memoryssa, ssa
from qbopt.model import mir
from qbopt.objectfile.module import Space


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


def reused(body: mir.MirBody, dgroup: frozenset[int] = frozenset()) -> mir.MirBody:
    predecessors = loops.predecessors(body.blocks)
    if not any(len(parents) > 1 for parents in predecessors.values()):
        return body
    graph = memoryssa.built(body)
    providers = [(site, value) for site, op in graph.operations.items() if (value := _value(op)) is not None]
    dominators = loops.dominators(body.blocks, body.entry)
    natural_loops = loops.loops(body.blocks, body.entry)
    fresh = max((value.id for value in ssa.values(body)), default=0) + 1
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
            if loaded is not None:
                ref, result = loaded
                site = memoryssa.Site(block.at, index)
                for parent in sorted(parents):
                    candidates = [(source, value) for source, (cell, value) in providers
                                  if source.block != block.at and source.block in dominators[parent]
                                  and block.at not in dominators[source.block]
                                  and value.width == result.width and graph.pointers.same_bytes(cell, ref)
                                  and all(source.block not in loop.body or block.at in loop.body for loop in natural_loops)
                                  and graph.available_on_edge(source, site, parent, ref, dgroup)]
                    if not candidates:
                        break
                    _, value = max(candidates, key=lambda item: (len(dominators[item[0].block]), item[0].index))
                    incoming[parent] = value.value
            if len(incoming) != len(parents):
                ops.append(op)
                continue
            value = mir.Value(fresh, block.at)
            fresh += 1
            phis.append(mir.Phi(value, incoming))
            ops.append(replace(op, kind=mir.Kind.COPY, args=(mir.Held(value, result.width),),
                               uses=(value,), loads=(), node=None, made=None, raised=None, symbol=False))
        blocks.append(replace(block, phis=tuple(phis), ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))
