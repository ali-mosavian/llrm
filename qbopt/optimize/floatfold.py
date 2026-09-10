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


def checks(body: mir.MirBody) -> mir.MirBody:
    """A completed observation stays satisfied until floating or unknown work."""
    transparent = {mir.Kind.NOTHING, mir.Kind.COPY, mir.Kind.LOAD, mir.Kind.STORE,
                   mir.Kind.ARG, mir.Kind.ADD, mir.Kind.SUB, mir.Kind.INCREMENT}
    blocks = []
    for block in body.blocks:
        observed = False
        ops = []
        for op in block.ops:
            if (op.barrier or op.floating is not None or op.stack is not None
                or any(isinstance(arg, mir.Held) and arg.width == 10 for arg in (*op.args, *op.results))):
                observed = False
            elif op.kind is mir.Kind.FCHECK:
                if observed:
                    op = replace(op, kind=mir.Kind.NOTHING, node=None, made=None, raised=None, symbol=False)
                observed = True
            elif op.kind not in transparent:
                observed = False
            ops.append(op)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


def stored(body: mir.MirBody, facts: dict) -> mir.MirBody:
    """Write exact SINGLE bits and retain checks for the now-unused computation."""
    if not facts:
        return body
    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            if (op.kind is mir.Kind.FSTORE and op.floating is not None
                and op.floating.result is Format.BINARY32 and not op.barrier
                and len(op.stores) == len(op.args) == 1 and not op.defines
                and isinstance(op.args[0], mir.Held) and op.args[0].value in facts):
                ref = op.stores[0]
                value = floatfacts.evaluated(op.kind, op.floating, (facts[op.args[0].value],))
                bits = floatfacts.encoded(value, Format.BINARY32) if value is not None else None
                if bits is not None and ref.width == 4 and ref.base is None and ref.segment is None and ref.addr is not None:
                    ops.append(_checked(op))
                    ops.append(replace(op, kind=mir.Kind.STORE, name="", args=(mir.Const(bits, 4),),
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
        for index in range(1, len(ops)):
            load, conversion = ops[index - 1:index + 1]
            if (conversion.kind is not mir.Kind.FSTORE or conversion.stores or conversion.barrier
                or len(conversion.results) != 1 or len(conversion.args) != 1):
                continue
            source, result = conversion.args[0], conversion.results[0]
            if (not isinstance(result, mir.Held) or result.value not in converted or reads[result.value]
                or not isinstance(source, mir.Held) or source.width != 10 or reads[source.value] != 1):
                continue
            if (load.kind is not mir.Kind.FLOAD or load.barrier or load.stores
                or load.results != (source,) or len(load.args) != 1
                or not isinstance(load.args[0], mir.Cell)
                or any(reads[value] for value in conversion.defines if value != result.value)
                or any(reads[value] for value in load.defines if value != source.value)):
                continue
            for position in (index - 1, index):
                ops[position] = _checked(ops[position])
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))
