"""Clone loop iterations as CFGs, retaining a residual loop and every exit.

This is the MIR building block for peeling and bounded full unrolling, not a
profitability decision. Callers must simplify and validate layout before using
the candidate for emission. Loop live-outs must already be in LCSSA.
"""

from dataclasses import replace

from qbopt.analysis import loops, ssa
from qbopt.model import mir
from qbopt.optimize import edges


def peeled(body: mir.MirBody, loop: loops.Loop, count: int) -> mir.MirBody | None:
    if count < 1:
        raise ValueError("peeling needs a positive iteration count")
    blocks = {block.at: block for block in body.blocks}
    predecessors = loops.predecessors(body.blocks)
    if loop.header == body.entry or len(loop.latches) != 1:
        return None
    outside = set(predecessors[loop.header]) - loop.body
    if len(outside) != 1:
        return None
    entry, = outside
    latch, = loop.latches
    if blocks[entry].succ != (loop.header,) or blocks[latch].succ != (loop.header,):
        return None
    if any(set(predecessors[at]) - loop.body for at in loop.body if at != loop.header):
        return None
    originals = [block for block in body.blocks if block.at in loop.body]
    if any(len(block.succ) > 1 and (
        len(block.succ) != 2 or not block.ops or block.ops[-1].kind is not mir.Kind.BRANCH
        or block.ops[-1].target not in block.succ
    ) for block in originals):
        return None
    defined = {value for block in originals for value in
               (*[phi.result for phi in block.phis],
                *[value for op in block.ops for value in op.defines])}
    for block in body.blocks:
        if block.at in loop.body:
            continue
        if any(value in defined for op in block.ops for value in op.uses):
            return None
        if any(value in defined and source not in loop.body
               for phi in block.phis for source, value in phi.incoming.items()):
            return None

    all_values = tuple(ssa.values(body))
    next_id = max((value.id for value in all_values), default=0) + 1
    next_variable = max((value.variable for value in all_values), default=0) + 1
    next_label = edges.fresh(body)
    labels, copies = [], []
    for _ in range(count):
        labels.append({block.at: next_label + index for index, block in enumerate(originals)})
        next_label += len(originals)
        copies.append({})
        for value in sorted(defined, key=lambda value: value.id):
            copies[-1][value.id] = replace(value, id=next_id, variable=next_variable, version=1)
            next_id += 1
            next_variable += 1

    def value(original, iteration):
        return copies[iteration].get(original.id, original)

    def destination(at, source, iteration):
        if source == latch and at == loop.header:
            return labels[iteration + 1][at] if iteration + 1 < count else at
        return labels[iteration].get(at, at)

    cloned = []
    for iteration in range(count):
        for block in originals:
            phis = []
            for phi in block.phis:
                if block.at == loop.header:
                    incoming = ({entry: phi.incoming[entry]} if iteration == 0 else
                                {labels[iteration - 1][latch]: value(phi.incoming[latch], iteration - 1)})
                else:
                    incoming = {labels[iteration][source]: value(incoming, iteration)
                                for source, incoming in phi.incoming.items()}
                phis.append(mir.Phi(value(phi.result, iteration), incoming))
            ops = []
            for op in block.ops:
                read = ssa.substituted(op, copies[iteration])
                ops.append(replace(
                    read, defines=tuple(value(result, iteration) for result in op.defines),
                    results=tuple(replace(result, value=value(result.value, iteration))
                                  if isinstance(result, mir.Held) else result for result in read.results),
                    target=destination(op.target, block.at, iteration),
                    covers=(op.at, op.at), extra_covers=(), raised=None,
                    symbol=op.symbol is not False,
                ))
            cloned.append(mir.MirBlock(labels[iteration][block.at], tuple(phis), tuple(ops),
                                      tuple(destination(at, block.at, iteration) for at in block.succ)))

    changed = []
    for block in body.blocks:
        if block.at == entry:
            block = replace(block, succ=(labels[0][loop.header],), ops=tuple(
                replace(op, target=labels[0][loop.header]) if op.target == loop.header else op
                for op in block.ops))
        phis = []
        for phi in block.phis:
            incoming = dict(phi.incoming)
            if block.at == loop.header:
                del incoming[entry]
                incoming[labels[-1][latch]] = value(phi.incoming[latch], count - 1)
            elif block.at not in loop.body:
                for source, original in phi.incoming.items():
                    if source in loop.body:
                        incoming.update({labels[iteration][source]: value(original, iteration)
                                         for iteration in range(count)})
            phis.append(replace(phi, incoming=incoming))
        changed.append(replace(block, phis=tuple(phis)))
    # Copy opaque allocation provenance without interpreting physical locations.
    def metadata(original):
        return {**original, **{value(old, iteration): location for old, location in original.items()
                              if old in defined for iteration in range(count)}}
    return replace(body, blocks=(*changed, *cloned),
                   origin=metadata(body.origin), pins=metadata(body.pins))
