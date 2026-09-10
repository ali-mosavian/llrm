"""Discard exact, unused conversions while retaining floating observation points."""

from collections import Counter
from dataclasses import replace

from qbopt.model import mir


def discarded(body: mir.MirBody, converted: dict) -> mir.MirBody:
    if not converted:
        return body
    reads = Counter(value for block in body.blocks for op in block.ops for value in op.uses)
    reads.update(value for block in body.blocks for phi in block.phis for value in phi.incoming.values())
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
                ops[position] = replace(ops[position], kind=mir.Kind.FCHECK, name="",
                    args=(), results=(), uses=(), defines=(), loads=(), stores=(), merges={},
                    node=None, made=None, raised=None, floating=None, stack=None, symbol=False)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))
