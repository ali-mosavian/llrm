from dataclasses import replace

from qbopt import loops
from qbopt import mir
from qbopt.module import Space


def sunk_stores(body: mir.MirBody, dgroup: frozenset[int], bounds: dict | None = None) -> mir.MirBody:
    predecessors = loops.predecessors(body.blocks)
    for loop in loops.loops(body.blocks, body.entry):
        blocks = {block.at: block for block in body.blocks}
        inside = [blocks[at] for at in loop.body]
        exits = {(block.at, to) for block in inside for to in block.succ if to not in loop.body}
        if len(exits) != 1:
            continue
        source, destination = next(iter(exits))
        if destination not in blocks or predecessors[destination] != frozenset({source}):
            continue
        if any(not block.succ for block in inside):
            continue
        operations = [op for block in inside for op in block.ops]
        if any(
            op.kind in {mir.Kind.CALL, mir.Kind.ARG, mir.Kind.OPAQUE, mir.Kind.ESCAPE, mir.Kind.RETURN}
            for op in operations
        ):
            continue
        exit_block = blocks[destination]
        if not exit_block.ops:
            continue
        moved = [op for op in blocks[source].ops if _unobserved(op, operations, dgroup, bounds)]
        if not moved:
            continue
        identities = {id(op) for op in moved}
        updates = {
            source: replace(blocks[source], ops=tuple(op for op in blocks[source].ops if id(op) not in identities)),
            destination: replace(
                exit_block,
                ops=tuple(replace(op, at=exit_block.ops[0].at) for op in moved) + exit_block.ops,
            ),
        }
        body = replace(body, blocks=tuple(updates.get(block.at, block) for block in body.blocks))
    return body


def _unobserved(op: mir.Op, operations: list[mir.Op], dgroup: frozenset[int], bounds: dict | None) -> bool:
    if op.kind is not mir.Kind.STORE or op.loads or op.defines or len(op.stores) != 1:
        return False
    ref = op.stores[0]
    if ref.addr is None or ref.addr.space is not Space.SEGMENT or ref.base is not None or ref.segment is not None:
        return False
    return not any(
        mir.overlapping(ref, other, dgroup, bounds)
        for one in operations
        if one is not op
        for other in (*one.loads, *one.stores)
    )
