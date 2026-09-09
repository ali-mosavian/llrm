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
            op.barrier or op.kind in {mir.Kind.CALL, mir.Kind.ARG, mir.Kind.OPAQUE, mir.Kind.ESCAPE, mir.Kind.RETURN}
            for op in operations
        ):
            continue
        exit_block = blocks[destination]
        if not exit_block.ops:
            continue
        moved = [op for op in blocks[source].ops if _unobserved(op, operations, dgroup, bounds)]
        relocated = {id(op): op for op in moved}
        if len(loop.latches) == 1 and source == loop.header:
            latch = next(iter(loop.latches))
            outside = predecessors[source] - loop.body
            if len(outside) == 1 and latch != source and blocks[latch].succ == (source,):
                entry = next(iter(outside))
                for op in blocks[latch].ops:
                    if _unobserved(op, operations, dgroup, bounds):
                        value = _exit_value(op, blocks[source], blocks[entry], latch, body, dgroup, bounds)
                        if value is not None:
                            moved.append(op)
                            relocated[id(op)] = replace(
                                op,
                                args=tuple(value if isinstance(arg, mir.Held) else arg for arg in op.args),
                                uses=(value.value,),
                            )
        if not moved:
            continue
        identities = {id(op) for op in moved}
        updates = {
            **{
                block.at: replace(block, ops=tuple(op for op in block.ops if id(op) not in identities))
                for block in inside
            },
            destination: replace(
                exit_block,
                ops=tuple(replace(relocated[id(op)], at=exit_block.ops[0].at) for op in moved) + exit_block.ops,
            ),
        }
        body = replace(body, blocks=tuple(updates.get(block.at, block) for block in body.blocks))
    return body


def _exit_value(
    op: mir.Op,
    header: mir.MirBlock,
    entry: mir.MirBlock,
    latch: int,
    body: mir.MirBody,
    dgroup: frozenset[int],
    bounds: dict | None,
) -> mir.Held | None:
    values = [arg for arg in op.args if isinstance(arg, mir.Held)]
    if len(values) != 1:
        return None
    stored = values[0]
    definitions = {value: operation for block in body.blocks for operation in block.ops for value in operation.defines}

    def root(arg: mir.Arg) -> mir.Arg:
        seen = set()
        while isinstance(arg, mir.Held) and arg.value not in seen:
            seen.add(arg.value)
            defining = definitions.get(arg.value)
            if defining is None or defining.kind is not mir.Kind.COPY or len(defining.args) != 1:
                break
            if not any(
                isinstance(result, mir.Held) and result.value == arg.value and result.width >= arg.width
                for result in defining.results
            ):
                break
            source = defining.args[0]
            if not isinstance(source, (mir.Held, mir.Const)) or source.width < arg.width:
                break
            arg = (
                mir.Held(source.value, arg.width)
                if isinstance(source, mir.Held)
                else mir.Const(source.n & ((1 << (arg.width * 8)) - 1), arg.width)
            )
        return arg

    ref = op.stores[0]
    blocks = {block.at: block for block in body.blocks}
    predecessors = loops.predecessors(body.blocks)

    def stored_at(at: int, expected: mir.Arg, active: frozenset) -> bool:
        expected = root(expected)
        key = (at, expected)
        if key in active:
            return True  # inductive backedge; every entry path still needs a matching store
        block = blocks[at]
        for previous in reversed(block.ops):
            if previous.barrier or previous.kind in {mir.Kind.CALL, mir.Kind.ESCAPE, mir.Kind.OPAQUE}:
                return False
            if any(mir.overlapping(ref, written, dgroup, bounds) for written in previous.stores):
                args = [arg for arg in previous.args if isinstance(arg, (mir.Const, mir.Held))]
                return (
                    previous.kind is mir.Kind.STORE
                    and previous.stores == (ref,)
                    and len(args) == 1
                    and root(args[0]) == expected
                )
        if at == body.entry or not predecessors[at]:
            return False
        phi = next((phi for phi in block.phis if isinstance(expected, mir.Held) and phi.result == expected.value), None)
        if phi is not None and set(phi.incoming) != predecessors[at]:
            return False
        return all(
            stored_at(
                parent,
                mir.Held(phi.incoming[parent], expected.width) if phi is not None else expected,
                active | {key},
            )
            for parent in predecessors[at]
        )

    for phi in header.phis:
        if set(phi.incoming) != {entry.at, latch} or root(mir.Held(phi.incoming[latch], stored.width)) != root(stored):
            continue
        seed = root(mir.Held(phi.incoming[entry.at], stored.width))
        if stored_at(entry.at, seed, frozenset()):
            return mir.Held(phi.result, stored.width)
    return None


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
