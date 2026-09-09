from dataclasses import replace

from qbopt import mir


def simplified(body: mir.MirBody, wanted: set[mir.Value], wide: set[mir.Value]) -> mir.MirBody:
    mentioned = {value for block in body.blocks for op in block.ops for value in op.uses if value not in op.merges} | {
        value for block in body.blocks for phi in block.phis for value in phi.incoming.values()
    }
    mentioned |= {value for block in body.blocks for op in block.ops for value in _operands_read(op)}
    changed = replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(
                    _simplified(_product(op, wanted | mentioned, wide), wanted | mentioned, wide) for op in block.ops
                ),
            )
            for block in body.blocks
        ),
    )
    removed = {value for block in body.blocks for op in block.ops for value in op.defines} - {
        value for block in changed.blocks for op in block.ops for value in op.defines
    }
    if not removed:
        return changed
    return replace(
        changed,
        blocks=tuple(
            replace(
                block,
                ops=tuple(
                    replace(
                        op,
                        uses=tuple(value for value in op.uses if value not in removed or value not in op.merges),
                        merges={source: target for source, target in op.merges.items() if source not in removed},
                    )
                    for op in block.ops
                ),
            )
            for block in changed.blocks
        ),
    )


def _product(op: mir.Op, wanted: set[mir.Value], wide: set[mir.Value]) -> mir.Op:
    if op.kind is not mir.Kind.MUL or op.barrier or op.stores or len(op.results) != 2 or len(op.args) != 2:
        return op
    if not all(isinstance(result, mir.Held) and result.width == 2 for result in op.results):
        return op
    if not all(
        (isinstance(arg, (mir.Held, mir.Const)) and arg.width == 2)
        or (isinstance(arg, mir.Cell) and arg.ref.width == 2)
        for arg in op.args
    ):
        return op
    result = op.results[0]
    if result.value in wide or any(value != result.value and value in wanted for value in op.defines):
        return op
    return replace(
        op,
        results=(result,),
        defines=(result.value,),
        uses=tuple(value for value in op.uses if value not in op.merges or value in _operands_read(op)),
        merges={},
        made=None,
    )


def _operands_read(op: mir.Op) -> set[mir.Value]:
    return {arg.value for arg in op.args if isinstance(arg, mir.Held)} | {
        value for ref in (*op.loads, *op.stores) for value in (ref.base, ref.segment) if value is not None
    }


def _simplified(op: mir.Op, wanted: set[mir.Value], wide: set[mir.Value]) -> mir.Op:
    if op.loads or op.stores or op.barrier or len(op.results) != 1 or len(op.args) != 2:
        return op
    result = op.results[0]
    if not isinstance(result, mir.Held) or result.width not in (2, 4):
        return op
    if op.kind is mir.Kind.CONCAT:
        high, low = op.args
        if isinstance(high, mir.Const) and isinstance(low, mir.Const) and high.width + low.width == result.width:
            number = ((high.n & ((1 << (high.width * 8)) - 1)) << (low.width * 8)) | (low.n & ((1 << (low.width * 8)) - 1))
            return replace(op, kind=mir.Kind.COPY, args=(mir.Const(number, result.width),), uses=(), made=None)
        return op
    if result.width == 2 and result.value in wide:
        return op
    if any(value != result.value and value in wanted for value in op.defines):
        return op
    left, right = op.args
    if not all(isinstance(arg, (mir.Held, mir.Const)) for arg in (left, right)):
        return op
    shift = op.kind in (mir.Kind.SHL, mir.Kind.SHR, mir.Kind.SAR)
    if left.width != result.width or (right.width != result.width and not (shift and isinstance(right, mir.Const))):
        return op
    if op.kind in (mir.Kind.ADD, mir.Kind.MUL, mir.Kind.AND, mir.Kind.OR, mir.Kind.XOR) and isinstance(left, mir.Const):
        left, right = right, left
    if not isinstance(right, mir.Const):
        return op
    mask = (1 << (result.width * 8)) - 1
    number = right.n & ((1 << (right.width * 8)) - 1)
    match op.kind, number:
        case mir.Kind.ADD | mir.Kind.SUB | mir.Kind.OR | mir.Kind.XOR | mir.Kind.SHL | mir.Kind.SHR | mir.Kind.SAR, 0:
            answer = left
        case mir.Kind.MUL, 1:
            answer = left
        case mir.Kind.AND, _ if number == mask:
            answer = left
        case mir.Kind.MUL | mir.Kind.AND, 0:
            answer = mir.Const(0, result.width)
        case mir.Kind.OR, _ if number == mask:
            answer = mir.Const(mask, result.width)
        case _:
            return op
    return replace(
        op,
        kind=mir.Kind.COPY,
        args=(answer,),
        defines=(result.value,),
        uses=(answer.value,) if isinstance(answer, mir.Held) else (),
        merges={},
        made=None,
    )
