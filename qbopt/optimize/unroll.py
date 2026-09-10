"""Bounded full unrolling of small constant-trip floating loops.

The MIR sequence is expanded in execution order. Reusing input provenance
does not give the emitter permission to sort it or discard cloned relocations.
"""

from dataclasses import replace

from qbopt.analysis import consts, induction, loops, ssa
from qbopt.model import mir
from qbopt.model.passes import MIRTransform, Where


class Unroll(MIRTransform):
    name = "unroll"

    def __init__(self, where: Where):
        self.where = where

    def transform(self, body):
        return expanded(body, self.where.dgroup, self.where.calls)


def expanded(body: mir.MirBody, dgroup: frozenset[int], calls: dict) -> mir.MirBody:
    blocks = {block.at: block for block in body.blocks}
    predecessors = loops.predecessors(body.blocks)
    facts = consts.known(body, dgroup, calls)
    for loop in loops.loops(body.blocks, body.entry):
        if len(loop.latches) != 1:
            continue
        header = blocks[loop.header]
        latch = blocks[next(iter(loop.latches))]
        outside = set(predecessors[header.at]) - loop.body
        exits = set(header.succ) - loop.body
        if len(outside) != 1 or len(exits) != 1 or latch.phis:
            continue
        entry, = outside
        exit_at, = exits
        if set(predecessors[exit_at]) != {header.at} or blocks[exit_at].phis:
            continue
        bridges = [blocks[at] for at in loop.body if at not in (header.at, latch.at)]
        if any(block.phis or len(block.succ) != 1 or any(
            op.kind not in (mir.Kind.NOTHING, mir.Kind.JUMP) for op in block.ops) for block in bridges):
            continue
        path = set()
        at, = set(header.succ) & loop.body
        while at != latch.at and at not in path:
            path.add(at)
            at = blocks[at].succ[0]
        if at != latch.at or path != {block.at for block in bridges}:
            continue
        if any(op.barrier or op.kind in (mir.Kind.OPAQUE, mir.Kind.BRANCH, mir.Kind.JUMP)
               for op in latch.ops):
            continue
        if not any(op.floating for op in latch.ops):
            continue
        if any(op.kind not in (mir.Kind.NOTHING, mir.Kind.STORE, mir.Kind.SUB, mir.Kind.BRANCH)
               or op.barrier or op.floating for op in header.ops):
            continue
        counts = set()
        for counter in induction.basics(body, loop).values():
            width = counter.start.width
            start = induction._signed(counter.start, facts, width)
            step = induction._signed(counter.step, facts, width)
            last = induction._last_counter(body, loop, counter, facts, width)
            if start is not None and step and last is not None:
                counts.add((last - start) // step + 1)
        if len(counts) != 1:
            continue
        count, = counts
        if not 2 <= count <= 4 or count * (len(latch.ops) + len(header.ops)) > 256:
            continue
        if any(set(phi.incoming) != {entry, latch.at} for phi in header.phis):
            continue
        return _expanded(body, loop, header, latch, exit_at, entry, count)
    return body


def _expanded(body, loop, header, latch, exit_at, entry, count):
    values = tuple(ssa.values(body))
    next_id = max(value.id for value in values) + 1
    next_variable = max(value.variable for value in values) + 1
    origin, pins = dict(body.origin), dict(body.pins)
    swap = {phi.result.id: phi.incoming[entry] for phi in header.phis}
    initial = dict(swap)

    def clone(op, owns):
        nonlocal next_id, next_variable
        read = ssa.substituted(op, swap)
        defined = {}
        for value in op.defines:
            fresh = replace(value, id=next_id, variable=next_variable, version=1)
            next_id += 1
            next_variable += 1
            defined[value.id] = fresh
            if value in origin:
                origin[fresh] = origin[value]
            if value in pins:
                pins[fresh] = pins[value]
        swap.update(defined)
        results = tuple(replace(arg, value=defined.get(arg.value.id, arg.value))
                        if isinstance(arg, mir.Held) else arg for arg in read.results)
        return replace(read, defines=tuple(defined[value.id] for value in op.defines), results=results,
                       covers=op.covers if owns else (op.at, op.at),
                       extra_covers=op.extra_covers if owns else (), raised=None,
                       symbol=op.symbol if owns else op.symbol is not False)

    expanded = []
    for iteration in range(count):
        if iteration:
            expanded.extend(clone(op, False) for op in header.ops[:-1])
        expanded.extend(clone(op, iteration == 0) for op in latch.ops)
        carried = {phi.result.id: ssa.provider(phi.incoming[latch.at], swap) for phi in header.phis}
        swap.update(carried)
    expanded.extend(clone(op, False) for op in header.ops[:-1])
    anchor = latch.ops[-1].at
    expanded.append(replace(header.ops[-1], at=anchor, kind=mir.Kind.JUMP, name="",
                            args=(), results=(), uses=(), defines=(), loads=(), stores=(),
                            merges={}, node=None, made=None, raised=((), ()),
                            covers=(anchor, anchor), extra_covers=(), target=exit_at, test=None, symbol=False))
    changed = []
    dominators = loops.dominators(body.blocks, body.entry)
    for block in body.blocks:
        if block.at == header.at:
            block = replace(block, phis=(), ops=tuple(ssa.substituted(op, initial) for op in block.ops))
        elif block.at == latch.at:
            block = replace(block, ops=tuple(expanded), succ=(exit_at,))
        elif block.at not in loop.body:
            # Only dominated exits read the final iteration's definitions.
            if exit_at in dominators.get(block.at, ()):
                block = replace(block, ops=tuple(ssa.substituted(op, swap) for op in block.ops),
                                phis=tuple(replace(phi, incoming={at: ssa.provider(value, swap)
                                           for at, value in phi.incoming.items()}) for phi in block.phis))
        changed.append(block)
    return replace(body, blocks=tuple(changed), origin=origin, pins=pins,
                   repetitions=(*body.repetitions, (latch.at, count)))
