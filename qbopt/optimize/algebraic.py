from collections import Counter
from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import consts


def simplified(body: mir.MirBody, wanted: set[mir.Value], wide: set[mir.Value]) -> mir.MirBody:
    from qbopt.optimize import wholephis
    from qbopt.optimize import wholestores
    body = wholestores.joined(wholephis.joined(body))
    body = _halved(_divisions(body))
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
        op = _redundant_extension(op, definitions)
        op = _zero_difference(op, definitions)
        op = _negated_difference(op, definitions, wanted | mentioned, uses)
        op = _shift_chain(op, definitions, wanted | mentioned, uses)
        op = _product(op, wanted | mentioned, wide)
        op = _scaled_chain(op, definitions, wanted | mentioned, uses)
        op = _offset_chain(op, definitions, wanted | mentioned, uses)
        op = _bitwise_chain(op, definitions, wanted | mentioned, uses)
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
                   source_backed=False, raised=None)


def _shared_shifts(body: mir.MirBody, wanted: set[mir.Value]) -> mir.MirBody:
    """Reuse a smaller available scale instead of shifting the original again."""
    used = {value for block in body.blocks for op in block.ops for value in _operands_read(op)}
    used |= {value for block in body.blocks for phi in block.phis for value in phi.incoming.values()}
    blocks = []
    for block in body.blocks:
        available = {}
        ops = []
        for op in block.ops:
            scale = _scale(op, wanted, tied=True) if op.kind is mir.Kind.SHL else None
            if scale is not None:
                source, factor = scale
                count = factor.bit_length() - 1
                candidates = available.setdefault(source, {})
                smaller = [amount for amount in candidates if amount < count]
                result = op.results[0]
                if smaller:
                    amount = max(smaller)
                    previous = candidates[amount]
                    op = replace(
                        op,
                        args=(previous, mir.Const(count - amount, 1)),
                        defines=(result.value,),
                        uses=tuple(previous.value if value == source.value else value for value in op.uses),
                        merges={
                            previous.value if value == source.value else value: target
                            for value, target in op.merges.items()
                        },
                        source_backed=False,
                        raised=None,
                    )
                if result.value in used:
                    candidates[count] = result
            ops.append(op)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


def _scale(op: mir.Op, wanted: set[mir.Value], tied: bool = False):
    if (op.kind not in (mir.Kind.MUL, mir.Kind.SHL) or op.loads or op.stores or op.barrier
        or op.merges and (not tied or mir.partial(op))
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
                   defines=(op.results[0].value,), uses=(source.value,), source_backed=False, raised=None)


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
                   defines=(op.results[0].value,), uses=(source.value,), source_backed=False, raised=None)


_ASSOCIATIVE_BITS = frozenset({mir.Kind.AND, mir.Kind.OR, mir.Kind.XOR})


def _bitwise(
    op: mir.Op, wanted: set[mir.Value], *, preserve_flags: bool = False
) -> tuple[mir.Held, int] | None:
    """A pure fixed-width bitwise operation with one constant operand."""
    if (
        op.kind not in _ASSOCIATIVE_BITS
        or op.loads
        or op.stores
        or op.barrier
        or op.merges
        or len(op.args) != 2
        or len(op.results) != 1
        or not isinstance(op.results[0], mir.Held)
    ):
        return None
    extra = tuple(value for value in op.defines if value != op.results[0].value)
    if any(not value.flags for value in extra) or (not preserve_flags and any(value in wanted for value in extra)):
        return None
    source, constant = op.args
    if isinstance(source, mir.Const):
        source, constant = constant, source
    result = op.results[0]
    if (
        not isinstance(source, mir.Held)
        or not isinstance(constant, mir.Const)
        or source.width != constant.width
        or source.width != result.width
    ):
        return None
    return source, consts.masked(constant.n, source.width)


def _bitwise_chain(op: mir.Op, definitions: dict, wanted: set[mir.Value], uses: Counter) -> mir.Op:
    """Compose single-use associative bitwise constants at one modular width."""
    # The final bitwise operation still computes identical flags from its
    # identical result.  An intermediate's flags would disappear and are only
    # admissible when unobserved.
    last = _bitwise(op, wanted, preserve_flags=True)
    if last is None:
        return op
    middle, constant = last
    previous = definitions.get(middle.value)
    if previous is None or previous.kind is not op.kind or uses[middle.value] != 1 or previous.results != (middle,):
        return op
    first = _bitwise(previous, wanted)
    if first is None:
        return op
    source, initial = first
    combined = consts.masked(consts.ARITH[op.kind](initial, constant), source.width)
    return replace(
        op,
        args=(source, mir.Const(combined, source.width)),
        uses=(source.value,),
        source_backed=False,
        raised=None,
    )


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
                   merges={}, source_backed=False, raised=None)


def _redundant_extension(op: mir.Op, definitions: dict) -> mir.Op:
    """Reuse bits an earlier same-kind extension has already established.

    A value zero-extended from 8 to 16 bits remains zero-extended when viewed
    through any 8..16-bit slice. Extending that view to at most 16 bits is a
    copy. The corresponding statement is true for sign extension because
    every added bit equals the original sign bit. Mixed signedness is not
    interchangeable and is deliberately excluded.
    """
    if (op.kind not in (mir.Kind.ZERO_EXTEND, mir.Kind.SIGN_EXTEND)
        or op.loads or op.stores or op.barrier or op.merges
        or len(op.args) != 1 or len(op.results) != 1
        or not isinstance(op.args[0], mir.Held)
        or not isinstance(op.results[0], mir.Held)
        or op.defines != (op.results[0].value,)):
        return op
    viewed, result = op.args[0], op.results[0]
    previous = definitions.get(viewed.value)
    if (previous is None or previous.kind is not op.kind
        or previous.loads or previous.stores or previous.barrier or previous.merges
        or len(previous.args) != 1 or len(previous.results) != 1
        or not isinstance(previous.results[0], mir.Held)
        or previous.results[0].value != viewed.value
        or previous.defines != (viewed.value,)):
        return op
    source_width = getattr(previous.args[0], "width", None)
    established = previous.results[0].width
    if source_width is None or not source_width <= viewed.width < result.width <= established:
        return op
    known = mir.Held(viewed.value, result.width)
    return replace(op, kind=mir.Kind.COPY, args=(known,), uses=(viewed.value,),
                   merges={}, source_backed=False, raised=None)


def _zero_difference(op: mir.Op, definitions: dict) -> mir.Op:
    """``0 - x`` is the unary modular negation of x, with identical flags."""
    if (op.kind is not mir.Kind.SUB or op.loads or op.stores or op.barrier or op.merges
        or len(op.args) != 2 or len(op.results) != 1
        or not isinstance(op.args[1], mir.Held) or not isinstance(op.results[0], mir.Held)
        or any(not value.flags for value in op.defines if value != op.results[0].value)):
        return op
    zero, source = op.args
    result = op.results[0]
    if (zero.width != source.width or source.width != result.width
        or set(op.uses) != {arg.value for arg in op.args if isinstance(arg, mir.Held)}
        or not _copied_zero(zero, definitions)):
        return op
    return replace(op, kind=mir.Kind.NEG, name="neg", op=ir.Operation.UNARY,
                   args=(source,), uses=(source.value,), source_backed=False, raised=None)


def _copied_zero(arg: mir.Arg, definitions: dict) -> bool:
    """Whether an operand is zero through width-preserving, effect-free copies."""
    width = arg.width
    seen = set()
    while isinstance(arg, mir.Held) and arg.value not in seen:
        seen.add(arg.value)
        made = definitions.get(arg.value)
        if (made is None or made.kind is not mir.Kind.COPY
            or made.loads or made.stores or made.barrier or made.merges
            or made.results != (arg,) or made.defines != (arg.value,)
            or len(made.args) != 1 or getattr(made.args[0], "width", None) != width):
            return False
        arg = made.args[0]
    return isinstance(arg, mir.Const) and arg.width == width and consts.masked(arg.n, width) == 0


def _halves(op: mir.Op):
    if (op.kind is not mir.Kind.CONCAT or op.loads or op.stores or op.barrier or len(op.args) != 2
        or not all(isinstance(arg, (mir.Held, mir.Const)) and arg.width == 2 for arg in op.args)
        or len(op.results) != 1 or not isinstance(op.results[0], mir.Held) or op.results[0].width != 4
        or op.defines != (op.results[0].value,)):
        return None
    return op.args


def _takes_halves(op: mir.Op, whole: mir.Held, readers: dict) -> bool:
    """Whether this reader of a joined value can read its two words instead."""
    if op.barrier or op.loads or op.merges:
        return False
    match op.kind:
        case mir.Kind.STORE:
            ref = op.stores[0] if len(op.stores) == 1 else None
            return (op.args == (whole,) and not op.defines and ref is not None and ref.width == 4
                    and ref.addr is not None and whole.value not in (ref.base, ref.segment))
        case mir.Kind.ARG:
            return op.args == (whole,) and not op.stores and not op.defines
        case mir.Kind.SUB:
            # Only the zero flag of `h | l` agrees with `whole - 0`.
            return (op.args == (whole, mir.Const(0, 4)) and not op.stores and not op.results
                    and all(value.flags for value in op.defines)
                    and all(reader.kind is mir.Kind.BRANCH and reader.test in (mir.Kind.EQ, mir.Kind.NE)
                            for value in op.defines for reader in readers.get(value, ())))
    return False


def _halved(body: mir.MirBody) -> mir.MirBody:
    """A value joined from two words, read only where words will do, is never joined."""
    joins = {op.results[0].value: halves for block in body.blocks for op in block.ops
             if (halves := _halves(op)) is not None}
    if not joins:
        return body
    readers: dict = {}
    for block in body.blocks:
        for op in block.ops:
            for value in _operands_read(op) | set(op.uses):
                readers.setdefault(value, []).append(op)
    phied = {value for block in body.blocks for phi in block.phis for value in phi.incoming.values()}
    split = {value: halves for value, halves in joins.items() if value not in phied and all(
        _takes_halves(op, mir.Held(value, 4), readers) for op in readers.get(value, ()))}
    if not split:
        return body
    values = {value for block in body.blocks for op in block.ops for value in (*op.defines, *op.uses)}
    values |= {value for block in body.blocks for phi in block.phis for value in (phi.result, *phi.incoming.values())}
    serial = max((value.id for value in values), default=0)
    variable = max((value.variable for value in values), default=0)

    def rewritten(op):
        nonlocal serial, variable
        whole = next((arg.value for arg in op.args if isinstance(arg, mir.Held) and arg.value in split), None)
        if whole is None:
            return (op,)
        high, low = split[whole]
        fresh = dict(source_backed=False, raised=None, merges={})
        later = dict(fresh, absorbed=(), id=None)

        def reads(*args, ref=None):
            held = [arg.value for arg in args if isinstance(arg, mir.Held)]
            return tuple(dict.fromkeys((*held, *(part for part in (ref.base, ref.segment) if part is not None)) if ref else held))
        match op.kind:
            case mir.Kind.STORE:
                ref = op.stores[0]
                words = ((low, replace(ref, width=2)), (high, replace(ref, addr=ref.addr.plus(2), width=2)))
                return tuple(replace(op, args=(word,), results=(mir.Cell(cell),), stores=(cell,),
                                     uses=reads(word, ref=cell), **(later if index else fresh))
                             for index, (word, cell) in enumerate(words))
            case mir.Kind.ARG:
                return tuple(replace(op, args=(word,), uses=reads(word), **(later if index else fresh))
                             for index, word in enumerate((high, low)))
            case mir.Kind.SUB:
                serial += 1
                variable += 1
                result = mir.Held(mir.Value(serial, op.at, variable=variable, version=1), 2)
                return (replace(op, kind=mir.Kind.OR, name="or", op=ir.Operation.BINARY, args=(high, low),
                                results=(result,), defines=(result.value, *op.defines), uses=reads(high, low), **fresh),)
        return (op,)

    return replace(body, blocks=tuple(
        replace(block, ops=tuple(one for op in block.ops for one in rewritten(op))) for block in body.blocks))


def _shift_chain(op: mir.Op, definitions: dict, wanted: set[mir.Value], uses: Counter) -> mir.Op:
    if op.kind is not mir.Kind.SHL or op.loads or op.stores or op.barrier or len(op.args) != 2 or len(op.results) != 1:
        return op
    source, count = op.args
    if not isinstance(source, mir.Held) or not isinstance(count, mir.Const):
        return op
    previous = definitions.get(source.value)
    if (
        previous is None
        or uses[source.value] != 1
        or previous.kind is not mir.Kind.SHL
        or len(previous.args) != 2
        or len(previous.results) != 1
    ):
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
                   source_backed=False, raised=None)


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
                or not all(isinstance(arg, mir.Held) and arg.width == op.args[0].width for arg in op.results)
                or not isinstance(op.args[0], mir.Held) or op.args[0].width not in (2, 4)
                or not isinstance(op.args[1], (mir.Held, mir.Const)) or op.args[1].width != op.args[0].width
                or set(op.defines) != {result.value for result in op.results}):
                ops.append(op)
                continue
            width = op.args[0].width
            bits = 8 * width
            fact = consts._operand(op, op.args[1], facts)
            divisor = consts.masked(fact.n, width) if fact is not None and fact.width >= width else 0
            if divisor <= 1 or divisor >= 1 << (bits - 1) or divisor & (divisor - 1):
                ops.append(op)
                continue
            shift = divisor.bit_length() - 1
            sequence = []

            def emit(kind, args, result=None):
                nonlocal serial, variable
                if result is None:
                    serial += 1
                    variable += 1
                    result = mir.Held(mir.Value(serial, op.at, variable=variable, version=1), width)
                sequence.append(mir.Op(
                    op.at, ir.Operation.BINARY, kind.value, (result.value,),
                    tuple(arg.value for arg in args if isinstance(arg, mir.Held)),
                    kind=kind, args=args, results=(result,),
                    id=op.id if not sequence else None,
                    absorbed=op.absorbed if not sequence else (),
                ))
                return result

            dividend = op.args[0]
            sign = emit(mir.Kind.SAR, (dividend, mir.Const(bits - 1, 1)))
            if divisor == 2:
                # The bias is the sign's low bit, 0 or 1: subtracting the sign word adds it.
                adjusted = emit(mir.Kind.SUB, (dividend, sign))
            else:
                bias = emit(mir.Kind.AND, (sign, mir.Const(divisor - 1, width)))
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
            return replace(op, kind=mir.Kind.COPY, args=(mir.Const(number, result.width),), uses=())
        return op
    if result.width == 2 and result.value in wide:
        return op
    if any(value != result.value and value in wanted for value in op.defines):
        return op
    left, right = op.args
    if not all(isinstance(arg, (mir.Held, mir.Const, mir.Symbol)) for arg in (left, right)):
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
        case mir.Kind.ADD | mir.Kind.SUB | mir.Kind.OR | mir.Kind.XOR | mir.Kind.SHL | mir.Kind.SHR | mir.Kind.SAR | mir.Kind.PTR_OFFSET, 0:
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
    )
