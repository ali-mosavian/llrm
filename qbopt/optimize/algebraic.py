from dataclasses import replace
from collections import Counter

from qbopt.analysis import consts
from qbopt.model import ir, mir


def simplified(body: mir.MirBody, wanted: set[mir.Value], wide: set[mir.Value]) -> mir.MirBody:
    body = _divisions(body)
    mentioned = {value for block in body.blocks for op in block.ops for value in op.uses if value not in op.merges} | {
        value for block in body.blocks for phi in block.phis for value in phi.incoming.values()
    }
    mentioned |= {value for block in body.blocks for op in block.ops for value in _operands_read(op)}
    definitions = {value: op for block in body.blocks for op in block.ops for value in op.defines}
    uses = Counter(value for block in body.blocks for op in block.ops
                   for value in _operands_read(op) | (set(op.uses) - op.merges.keys()))
    uses.update(value for block in body.blocks for phi in block.phis for value in phi.incoming.values())

    def simplify(op):
        op = _recombined(op, definitions)
        op = _negated_difference(op, definitions, wanted | mentioned, uses)
        op = _shift_chain(op, definitions, wanted | mentioned)
        op = _product(op, wanted | mentioned, wide)
        op = _scaled_chain(op, definitions, wanted | mentioned, uses)
        op = _offset_chain(op, definitions, wanted | mentioned, uses)
        return _simplified(op, wanted | mentioned, wide)

    changed = replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(
                    simplify(op)
                    for op in block.ops
                ),
            )
            for block in body.blocks
        ),
    )
    changed = _shared_shifts(changed, wanted | mentioned)
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


def _negated_difference(op: mir.Op, definitions: dict, wanted: set[mir.Value], uses: Counter) -> mir.Op:
    """Negating a single-use modular difference reverses its operands."""
    if (op.kind is not mir.Kind.NEG or op.loads or op.stores or op.barrier or op.merges
        or len(op.args) != 1 or len(op.results) != 1
        or not all(isinstance(arg, mir.Held) for arg in (*op.args, *op.results))):
        return op
    source, result = op.args[0], op.results[0]
    if source.width != result.width or uses[source.value] != 1:
        return op
    difference = definitions.get(source.value)
    if (difference is None or difference.kind is not mir.Kind.SUB or difference.loads or difference.stores
        or difference.barrier or difference.merges or difference.results != (source,)
        or len(difference.args) != 2
        or any(not isinstance(arg, (mir.Held, mir.Const)) or arg.width != result.width
               for arg in difference.args)
        or any(value in wanted for one in (op, difference) for value in one.defines
               if value != one.results[0].value)):
        return op
    args = tuple(reversed(difference.args))
    return replace(op, kind=mir.Kind.SUB, name="sub", op=ir.Operation.BINARY, args=args,
                   defines=(result.value,), uses=tuple(arg.value for arg in args if isinstance(arg, mir.Held)),
                   node=None, made=None, raised=None)


def _shared_shifts(body: mir.MirBody, wanted: set[mir.Value]) -> mir.MirBody:
    """Reuse a smaller available scale instead of shifting the original again."""
    used = {value for block in body.blocks for op in block.ops for value in _operands_read(op)}
    used |= {value for block in body.blocks for phi in block.phis for value in phi.incoming.values()}
    blocks = []
    for block in body.blocks:
        available = {}
        ops = []
        for op in block.ops:
            scale = _scale(op, wanted) if op.kind is mir.Kind.SHL else None
            if scale is not None:
                source, factor = scale
                count = factor.bit_length() - 1
                candidates = available.setdefault(source, {})
                smaller = [amount for amount in candidates if amount < count]
                result = op.results[0]
                if smaller:
                    amount = max(smaller)
                    previous = candidates[amount]
                    op = replace(op, args=(previous, mir.Const(count - amount, 1)),
                                 defines=(result.value,), uses=(previous.value,),
                                 node=None, made=None, raised=None)
                if result.value in used:
                    candidates[count] = result
            ops.append(op)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


def _scale(op: mir.Op, wanted: set[mir.Value]):
    if (op.kind not in (mir.Kind.MUL, mir.Kind.SHL) or op.loads or op.stores or op.barrier or op.merges
        or len(op.args) != 2 or len(op.results) != 1 or not isinstance(op.results[0], mir.Held)
        or any(value in wanted for value in op.defines if value != op.results[0].value)):
        return None
    source, factor = op.args
    if not isinstance(source, mir.Held) or not isinstance(factor, mir.Const) or source.width != op.results[0].width:
        return None
    if op.kind is mir.Kind.SHL:
        if not 0 < factor.n < source.width * 8:
            return None
        return source, 1 << factor.n
    return (source, factor.n) if factor.width == source.width else None


def _scaled_chain(op: mir.Op, definitions: dict, wanted: set[mir.Value], uses: Counter) -> mir.Op:
    """Combine single-use integer scales at an unchanged modular width."""
    last = _scale(op, wanted)
    if last is None:
        return op
    middle, factor = last
    previous = definitions.get(middle.value)
    if previous is None or uses[middle.value] != 1 or previous.results != (middle,):
        return op
    first = _scale(previous, wanted)
    if first is None:
        return op
    source, initial = first
    factor = consts.masked(initial * factor, source.width)
    return replace(op, kind=mir.Kind.MUL, args=(source, mir.Const(factor, source.width)),
                   defines=(op.results[0].value,), uses=(source.value,), node=None, made=None, raised=None)


def _offset(op: mir.Op, wanted: set[mir.Value]):
    if (op.kind not in (mir.Kind.ADD, mir.Kind.SUB) or op.loads or op.stores or op.barrier or op.merges
        or len(op.args) != 2 or len(op.results) != 1 or not isinstance(op.results[0], mir.Held)
        or any(value in wanted for value in op.defines if value != op.results[0].value)):
        return None
    source, amount = op.args
    if op.kind is mir.Kind.ADD and isinstance(source, mir.Const):
        source, amount = amount, source
    if (not isinstance(source, mir.Held) or not isinstance(amount, mir.Const)
        or source.width != amount.width or source.width != op.results[0].width):
        return None
    return source, amount.n if op.kind is mir.Kind.ADD else -amount.n


def _offset_chain(op: mir.Op, definitions: dict, wanted: set[mir.Value], uses: Counter) -> mir.Op:
    """Compose single-use modular offsets without preserving intermediate flags."""
    last = _offset(op, wanted)
    if last is None:
        return op
    middle, amount = last
    previous = definitions.get(middle.value)
    if previous is None or uses[middle.value] != 1 or previous.results != (middle,):
        return op
    first = _offset(previous, wanted)
    if first is None:
        return op
    source, initial = first
    amount = consts.masked(initial + amount, source.width)
    return replace(op, kind=mir.Kind.ADD, name="add", op=ir.Operation.BINARY,
                   args=(source, mir.Const(amount, source.width)),
                   defines=(op.results[0].value,), uses=(source.value,), node=None, made=None, raised=None)


def _recombined(op: mir.Op, definitions: dict) -> mir.Op:
    """Joining both extracted halves of one value is that value, without a round trip."""
    if (op.kind is not mir.Kind.CONCAT or op.loads or op.stores or op.barrier
        or len(op.args) != 2 or len(op.results) != 1
        or not isinstance(op.results[0], mir.Held) or op.results[0].width != 4
        or op.defines != (op.results[0].value,)):
        return op
    original = mir.extracted_whole(*op.args, definitions)
    if original is None:
        return op
    return replace(op, kind=mir.Kind.COPY, args=(original,), uses=(original.value,),
                   merges={}, node=None, made=None, raised=None)


def _shift_chain(op: mir.Op, definitions: dict, wanted: set[mir.Value]) -> mir.Op:
    if op.kind is not mir.Kind.SHL or op.loads or op.stores or op.barrier or len(op.args) != 2 or len(op.results) != 1:
        return op
    source, count = op.args
    if not isinstance(source, mir.Held) or not isinstance(count, mir.Const):
        return op
    previous = definitions.get(source.value)
    if previous is None or previous.kind is not mir.Kind.SHL or len(previous.args) != 2 or len(previous.results) != 1:
        return op
    original, first_count = previous.args
    if (not isinstance(original, mir.Held) or not isinstance(first_count, mir.Const)
        or previous.results[0] != source or op.results[0].width != source.width or original.width != source.width
        or any(value in wanted for value in op.defines if value != op.results[0].value)):
        return op
    total = first_count.n + count.n
    if min(first_count.n, count.n) <= 0 or total >= source.width * 8:
        return op
    return replace(op, args=(original, mir.Const(total, count.width)),
                   uses=tuple(dict.fromkeys(original.value if value == source.value else value for value in op.uses)),
                   node=None, made=None, raised=None)


def _divisions(body: mir.MirBody) -> mir.MirBody:
    """Divide by positive powers of two, biasing negatives to truncate toward zero."""
    if not any(op.kind is mir.Kind.DIVMOD for block in body.blocks for op in block.ops):
        return body
    facts = consts.known(body)
    values = {value for block in body.blocks for op in block.ops for value in (*op.defines, *op.uses)}
    values |= {value for block in body.blocks for phi in block.phis for value in (phi.result, *phi.incoming.values())}
    serial = max((value.id for value in values), default=0)
    variable = max((value.variable for value in values), default=0)
    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            if (op.kind is not mir.Kind.DIVMOD or op.loads or op.stores or op.barrier
                or len(op.args) != 2 or len(op.results) != 2
                or not all(isinstance(arg, mir.Held) and arg.width == 4 for arg in (op.args[0], *op.results))
                or not isinstance(op.args[1], (mir.Held, mir.Const)) or op.args[1].width != 4
                or set(op.defines) != {result.value for result in op.results}):
                ops.append(op)
                continue
            fact = consts._operand(op, op.args[1], facts)
            divisor = consts.masked(fact.n, 4) if fact is not None and fact.width >= 4 else 0
            if divisor <= 1 or divisor >= 0x80000000 or divisor & (divisor - 1):
                ops.append(op)
                continue
            shift = divisor.bit_length() - 1
            sequence = []

            def emit(kind, args, result=None):
                nonlocal serial, variable
                if result is None:
                    serial += 1
                    variable += 1
                    result = mir.Held(mir.Value(serial, op.at, variable=variable, version=1), 4)
                sequence.append(mir.Op(
                    op.at, ir.Operation.BINARY, kind.value, (result.value,),
                    tuple(arg.value for arg in args if isinstance(arg, mir.Held)),
                    kind=kind, args=args, results=(result,),
                    covers=op.covers if not sequence else (op.at, op.at),
                    id=op.id if not sequence else None,
                    extra_covers=op.extra_covers if not sequence else (),
                ))
                return result

            dividend = op.args[0]
            sign = emit(mir.Kind.SAR, (dividend, mir.Const(31, 1)))
            bias = emit(mir.Kind.AND, (sign, mir.Const(divisor - 1, 4)))
            adjusted = emit(mir.Kind.ADD, (dividend, bias))
            quotient = emit(mir.Kind.SAR, (adjusted, mir.Const(shift, 1)), op.results[0])
            product = emit(mir.Kind.SHL, (quotient, mir.Const(shift, 1)))
            emit(mir.Kind.SUB, (dividend, product), op.results[1])
            ops.extend(sequence)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


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
