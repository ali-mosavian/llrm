"""Discard exact, unused conversions while retaining floating observation points."""

from collections import Counter
from dataclasses import replace

from qbopt.model import mir
from qbopt.model.floating import Format
from qbopt.analysis import floatfacts


def _checked(op):
    return replace(op, kind=mir.Kind.FCHECK, name="",
        args=(), results=(), uses=(), defines=(), loads=(), stores=(), merges={},
        node=None, made=None, raised=None, floating=None, stack=None, symbol=False)


def _reads(body):
    reads = Counter(value for block in body.blocks for op in block.ops for value in op.uses)
    reads.update(value for block in body.blocks for phi in block.phis for value in phi.incoming.values())
    return reads


def _observed_after(op, observed):
    if (op.barrier or op.floating is not None or op.stack is not None
        or any(isinstance(arg, mir.Held) and arg.width == 10 for arg in (*op.args, *op.results))):
        return False
    if op.kind is mir.Kind.FCHECK:
        return True
    transparent = {mir.Kind.NOTHING, mir.Kind.COPY, mir.Kind.LOAD, mir.Kind.STORE,
                   mir.Kind.ARG, mir.Kind.ADD, mir.Kind.SUB, mir.Kind.INCREMENT,
                   mir.Kind.BRANCH, mir.Kind.JUMP, mir.Kind.LT, mir.Kind.LE,
                   mir.Kind.GT, mir.Kind.GE, mir.Kind.EQ, mir.Kind.NE}
    return observed and op.kind in transparent


def checks(body: mir.MirBody) -> mir.MirBody:
    """A completed observation stays satisfied until floating or unknown work."""
    predecessors = {block.at: [] for block in body.blocks}
    for block in body.blocks:
        for successor in block.succ:
            if successor in predecessors:
                predecessors[successor].append(block.at)
    entries = dict.fromkeys(predecessors, False)
    exits = dict(entries)
    changed = True
    while changed:
        changed = False
        for block in body.blocks:
            parents = predecessors[block.at]
            observed = bool(parents) and block.at != body.entry and all(exits[at] for at in parents)
            entries[block.at] = observed
            for op in block.ops:
                observed = _observed_after(op, observed)
            if exits[block.at] != observed:
                exits[block.at] = observed
                changed = True
    blocks = []
    for block in body.blocks:
        observed = entries[block.at]
        ops = []
        for op in block.ops:
            after = _observed_after(op, observed)
            if op.kind is mir.Kind.FCHECK and observed and after:
                op = replace(op, kind=mir.Kind.NOTHING, node=None, made=None, raised=None, symbol=False)
            observed = after
            ops.append(op)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


def stored(body: mir.MirBody, facts: dict) -> mir.MirBody:
    """Write exact storage bits and retain checks for the now-unused computation."""
    if not facts:
        return body
    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            if (op.kind is mir.Kind.FSTORE and op.floating is not None
                and op.floating.result in {Format.BINARY32, Format.BINARY64} and not op.barrier
                and len(op.stores) == len(op.args) == 1 and not op.defines
                and isinstance(op.args[0], mir.Held) and op.args[0].value in facts):
                ref = op.stores[0]
                value = floatfacts.evaluated(op.kind, op.floating, (facts[op.args[0].value],))
                format = op.floating.result
                width = 4 if format is Format.BINARY32 else 8
                bits = floatfacts.encoded(value, format) if value is not None else None
                if bits is not None and ref.width == width and ref.base is None and ref.segment is None and ref.addr is not None:
                    ops.append(_checked(op))
                    ops.append(replace(op, kind=mir.Kind.STORE, name="", args=(mir.Const(bits, width),),
                        results=(mir.Cell(ref),), uses=(), loads=(), merges={},
                        node=None, made=None, raised=None, floating=None, floating_origin=None,
                        stack=None, covers=(op.at, op.at), extra_covers=(), symbol=True))
                    continue
            ops.append(op)
        blocks.append(replace(block, ops=tuple(ops)))
    return _dead_values(replace(body, blocks=tuple(blocks)), facts)


def _dead_values(changed, facts):
    while True:
        reads = _reads(changed)
        removed = False
        blocks = []
        for block in changed.blocks:
            ops = []
            for op in block.ops:
                if (op.floating is not None and not op.stores and not op.barrier
                    and len(op.results) == 1 and isinstance(op.results[0], mir.Held)
                    and op.results[0].width == 10 and op.results[0].value in facts
                    and not any(reads[value] for value in op.defines)):
                    op = _checked(op)
                    removed = True
                ops.append(op)
            blocks.append(replace(block, ops=tuple(ops)))
        changed = replace(changed, blocks=tuple(blocks))
        if not removed:
            return changed


def discarded(body: mir.MirBody, converted: dict) -> mir.MirBody:
    if not converted:
        return body
    reads = _reads(body)
    blocks = []
    for block in body.blocks:
        ops = list(block.ops)
        for index, conversion in enumerate(ops):
            if (conversion.kind is not mir.Kind.FSTORE or conversion.stores or conversion.barrier
                or len(conversion.results) != 1 or len(conversion.args) != 1):
                continue
            source, result = conversion.args[0], conversion.results[0]
            if (not isinstance(result, mir.Held) or result.value not in converted or reads[result.value]
                or not isinstance(source, mir.Held) or source.width != 10 or reads[source.value] != 1):
                continue
            if any(reads[value] for value in conversion.defines if value != result.value):
                continue
            ops[index] = _checked(conversion)
            load = ops[index - 1] if index else None
            if (load is not None and load.kind is mir.Kind.FLOAD and not load.barrier and not load.stores
                and load.results == (source,) and len(load.args) == 1 and isinstance(load.args[0], mir.Cell)
                and not any(reads[value] for value in load.defines if value != source.value)):
                ops[index - 1] = _checked(load)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))
