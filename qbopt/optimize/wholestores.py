"""Store a whole value instead of separately storing its extracted words."""

from dataclasses import replace

from qbopt.model import ir, mir


def _word(op):
    return (op.kind is mir.Kind.STORE and not op.barrier and not op.defines and not op.loads
            and len(op.args) == len(op.stores) == 1
            and isinstance(op.args[0], mir.Held) and op.args[0].width == 2
            and op.stores[0].width == 2 and op.stores[0].addr is not None)


def joined(body: mir.MirBody) -> mir.MirBody:
    definitions = {value: op for block in body.blocks for op in block.ops for value in op.defines}
    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            low = ops[-1] if ops else None
            if low is not None and _word(low) and _word(op):
                ref = low.stores[0]
                if replace(ref, addr=ref.addr.plus(2)) == op.stores[0]:
                    whole = mir.extracted_whole(op.args[0], low.args[0], definitions)
                    if whole is not None:
                        ref = replace(ref, width=4)
                        uses = tuple(dict.fromkeys(value for value in (whole.value, ref.base, ref.segment)
                                                  if value is not None))
                        spans = tuple(dict.fromkeys((*low.extra_covers, *op.extra_covers,
                                                     *((op.covers,) if op.covers is not None else ()))))
                        ops[-1] = replace(low, op=ir.Operation.MOVE, name="mov", args=(whole,),
                                          results=(mir.Cell(ref),), stores=(ref,), uses=uses,
                                          merges={}, source_backed=False, raised=None, extra_covers=spans)
                        continue
            ops.append(op)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))
