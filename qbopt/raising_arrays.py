from dataclasses import replace

from qbopt import mir
from qbopt import consts


def annotated(body: mir.MirBody, calls: dict[int, str]) -> mir.MirBody:
    sites = {at: name for at, name in calls.items() if name in ("B$DDIM", "B$RDIM")}
    if not sites:
        return body
    known = consts.known(body)
    symbols: dict[mir.Value, mir.Symbol] = {}
    blocks = []
    for block in body.blocks:
        arguments: list[mir.Const | mir.Symbol | None] = []
        ops = []
        for op in block.ops:
            if op.kind is mir.Kind.COPY and len(op.args) == len(op.results) == 1:
                source = _argument(op.args[0], known, symbols)
                result = op.results[0]
                if isinstance(source, mir.Symbol) and isinstance(result, mir.Held) and result.width == source.width:
                    symbols[result.value] = source
            if op.kind is mir.Kind.ARG and len(op.args) == 1:
                arguments.append(_argument(op.args[0], known, symbols))
            elif op.at in sites and op.kind is mir.Kind.CALL:
                request = _request(arguments, sites[op.at] == "B$RDIM")
                op = replace(op, array=request)
                arguments.clear()
            elif op.kind not in (mir.Kind.COPY, mir.Kind.XOR) or op.stores or op.barrier:
                arguments.clear()
            ops.append(op)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


def _argument(
    arg: mir.Arg, known: dict[mir.Value, consts.Known], symbols: dict[mir.Value, mir.Symbol]
) -> mir.Const | mir.Symbol | None:
    if isinstance(arg, (mir.Const, mir.Symbol)):
        return arg if arg.width == 2 else None
    if not isinstance(arg, mir.Held) or arg.width != 2:
        return None
    if symbol := symbols.get(arg.value):
        return symbol
    value = known.get(arg.value)
    if value is None or value.width < arg.width:
        return None
    number = value.n & 0xFFFF
    return mir.Const(number if number < 0x8000 else number - 0x10000, 2)


def _request(arguments: list[mir.Const | mir.Symbol | None], replaces: bool) -> mir.ArrayRequest | None:
    # runtime/rt/dynamic.asm: lo1, hi1, ..., loN, hiN, element size,
    # dimension count plus attributes, descriptor. ADIM does not allocate.
    if len(arguments) < 5:
        return None
    width, dimensions, descriptor = arguments[-3:]
    if (
        not isinstance(width, mir.Const)
        or not isinstance(dimensions, mir.Const)
        or not isinstance(descriptor, mir.Symbol)
    ):
        return None
    count = dimensions.n & 0xFF
    if count == 0 or width.n <= 0 or len(arguments) != 2 * count + 3:
        return None
    bounds = []
    for index in range(count):
        lower, upper = arguments[index * 2 : index * 2 + 2]
        if not isinstance(lower, mir.Const) or not isinstance(upper, mir.Const) or upper.n < lower.n:
            return None
        bounds.append((lower.n, upper.n))
    return mir.ArrayRequest(descriptor, width.n, tuple(bounds), replaces)
