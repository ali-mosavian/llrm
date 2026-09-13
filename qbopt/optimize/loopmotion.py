from dataclasses import replace
from functools import cache

from qbopt.analysis import loops
from qbopt.model import mir
from qbopt.analysis import induction
from qbopt.analysis import ranges
from qbopt.analysis import consts
from qbopt.objectfile.module import Space


def sunk_stores(body: mir.MirBody, dgroup: frozenset[int], bounds: dict | None = None) -> mir.MirBody:
    predecessors = loops.predecessors(body.blocks)
    for loop in loops.loops(body.blocks, body.entry):
        scoped = ranges.bounded(body)
        # With constants, as hoist asks: a store through the literal selector
        # 0A000h otherwise observes every frame and descriptor cell.
        constant = ranges.constants(body, dgroup)
        intervals = {id(op): {**constant, **scoped.get(block.at, {})} for block in body.blocks for op in block.ops}
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
            op.floating is not None or op.barrier
            or op.kind in {mir.Kind.CALL, mir.Kind.ARG, mir.Kind.OPAQUE, mir.Kind.ESCAPE, mir.Kind.RETURN}
            for op in operations
        ):
            continue
        exit_block = blocks[destination]
        if not exit_block.ops:
            continue
        dominators = loops.dominators(body.blocks, body.entry)
        address_values = {value for block in body.blocks
                          if block.at not in loop.body and block.at in dominators.get(loop.header, ())
                          for value in (*[phi.result for phi in block.phis],
                                        *[value for op in block.ops for value in op.defines])}
        moved = [op for op in blocks[source].ops
                 if _unobserved(op, operations, dgroup, bounds, intervals, address_values)]
        relocated = {id(op): op for op in moved}
        if len(loop.latches) == 1 and source == loop.header:
            latch = next(iter(loop.latches))
            outside = predecessors[source] - loop.body
            if len(outside) == 1 and latch != source and blocks[latch].succ == (source,):
                entry = next(iter(outside))
                nonempty = induction.nonempty(body, loop)
                invariant = induction.invariant(body, set(loop.body)) if nonempty else set()
                for op in operations:
                    if id(op) not in relocated and _unobserved(op, operations, dgroup, bounds, intervals, address_values):
                        value = _exit_value(op, blocks[source], blocks[entry], latch, body, dgroup, bounds)
                        if value is None and any(op is one for one in blocks[latch].ops):
                            value = _invariant_value(op, invariant, nonempty)
                            if value is None:
                                value = _last_counter_value(op, body, loop)
                        if value is not None:
                            moved.append(op)
                            relocated[id(op)] = replace(
                                op,
                                args=tuple(value if isinstance(arg, mir.Held) else arg for arg in op.args),
                                uses=tuple(dict.fromkeys(([value.value] if isinstance(value, mir.Held) else [])
                                    + [part for ref in op.stores for part in (ref.base, ref.segment) if part is not None])),
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


def _last_counter_value(op: mir.Op, body: mir.MirBody, loop) -> mir.Const | None:
    if len(op.args) != 1 or not isinstance(op.args[0], mir.Held):
        return None
    stored = op.args[0]
    counter = induction.basics(body, loop).get(stored.value.id)
    if counter is None or counter.start.width != stored.width or op.stores[0].width != stored.width:
        return None
    last = induction._last_counter(body, loop, counter, consts.known(body), stored.width)
    return mir.Const(consts.masked(last, stored.width), stored.width) if last is not None else None


def _invariant_value(op: mir.Op, invariant: set[int], nonempty: bool) -> mir.Arg | None:
    if not nonempty or len(op.args) != 1:
        return None
    match op.args[0]:
        case mir.Const() as value:
            return value
        case mir.Held(value=value) as arg if value.id in invariant:
            return arg
    return None


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

    @cache
    def memory():
        barriers = {one.at: "" for block in body.blocks for one in block.ops
                    if one.barrier or one.kind in {mir.Kind.CALL, mir.Kind.ESCAPE, mir.Kind.OPAQUE}}
        return consts.cells(body, dgroup, barriers), barriers

    def stored_at(at: int, expected: mir.Arg, active: frozenset) -> bool:
        expected = root(expected)
        key = (at, expected)
        if key in active:
            return True  # inductive backedge; every entry path still needs a matching store
        block = blocks[at]
        for index in range(len(block.ops) - 1, -1, -1):
            previous = block.ops[index]
            if previous.barrier or previous.kind in {mir.Kind.CALL, mir.Kind.ESCAPE, mir.Kind.OPAQUE}:
                return False
            if any(mir.overlapping(ref, written, dgroup, bounds) for written in previous.stores):
                if isinstance(expected, mir.Const):
                    fact = consts.initialized(previous, ref)
                    if fact is None:
                        facts, barriers = memory()
                        after = consts._kills(facts.get((at, index), {}), previous, {}, dgroup, barriers)
                        fact = consts._cell(after, ref)
                    if fact == consts.Known(consts.masked(expected.n, expected.width), expected.width):
                        return True
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
        if set(phi.incoming) != {entry.at, latch}:
            continue
        seed = root(mir.Held(phi.incoming[entry.at], stored.width))
        if stored_at(entry.at, seed, frozenset()) and stored_at(latch, mir.Held(phi.incoming[latch], stored.width), frozenset()):
            return mir.Held(phi.result, stored.width)
    return None


def _unobserved(op: mir.Op, operations: list[mir.Op], dgroup: frozenset[int], bounds: dict | None,
                intervals: dict | None = None, address_values: set | frozenset = frozenset()) -> bool:
    if op.kind is not mir.Kind.STORE or op.loads or op.defines or len(op.stores) != 1:
        return False
    ref = op.stores[0]
    if ref.addr is None or ref.addr.space not in (Space.SEGMENT, Space.FRAME) or ref.segment is not None:
        return False
    if ref.base is not None and (ref.base not in address_values or not ref.excludes):
        return False
    return not any(
        mir.overlapping(ref, other, dgroup, bounds, known=(intervals or {}).get(id(op)),
                        other_known=(intervals or {}).get(id(one)))
        for one in operations
        if one is not op
        for other in (*one.loads, *one.stores)
    )
