from dataclasses import replace

from qbopt.model import mir
from qbopt.analysis import consts
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def annotated(body: mir.MirBody, calls: dict[int, str], *, family: str = "") -> mir.MirBody:
    sites = {at: name for at, name in calls.items() if name in ("B$DDIM", "B$RDIM")}
    if not sites:
        from qbopt.frontend import arrayfacts
        from qbopt.frontend import raising_fields

        return raising_fields.named(arrayfacts.proven(body))
    known = consts.known(body)
    symbols: dict[mir.Value, mir.Symbol] = {}
    blocks = []
    for block in body.blocks:
        arguments: list[mir.Const | mir.Symbol | None] = []
        ops = []
        for op in block.ops:
            if op.kind in (mir.Kind.COPY, mir.Kind.ADDRESS) and len(op.args) == len(op.results) == 1:
                source = _argument(op.args[0], known, symbols)
                result = op.results[0]
                if isinstance(source, mir.Symbol) and isinstance(result, mir.Held) and result.width == source.width:
                    symbols[result.value] = source
            if op.kind is mir.Kind.ARG and len(op.args) == 1:
                arguments.append(_argument(op.args[0], known, symbols))
            elif op.at in sites and op.kind is mir.Kind.CALL:
                request = _request(arguments, sites[op.at] == "B$RDIM")
                values = _descriptor_values(request, arguments, family) if sites[op.at] == "B$DDIM" else ()
                op = replace(op, array=request, memory_values=values)
                arguments.clear()
            elif op.kind not in (mir.Kind.COPY, mir.Kind.ADDRESS, mir.Kind.XOR) or op.stores or op.barrier:
                arguments.clear()
            ops.append(op)
        blocks.append(replace(block, ops=tuple(ops)))
    from qbopt.frontend import raising_array_bounds

    return raising_array_bounds.proven(_addresses(replace(body, blocks=tuple(blocks)), symbols))


def _descriptor_values(request: mir.ArrayRequest | None, arguments: list, family: str) -> tuple:
    """Normal-return facts for the numeric DDIM layout verified in the three shipped libraries."""
    shape = _shape(arguments)
    if shape is None or family not in ("qb45", "pds71", "vbdos"):
        return ()
    descriptor, element_width, rank, attributes = shape
    if attributes not in (0, 1, 2, 3):
        return ()
    start = descriptor.offset + descriptor.addend
    if descriptor.space is not Space.SEGMENT or not 0 <= start <= 65536 - (14 + 4 * rank):
        return ()
    # dynamic.asm consumes the stack backwards: last dimension comes first.
    fields = [(8, rank, 1), (9, attributes, 1), (12, element_width, 2)]
    for dimension, (lower, upper) in enumerate(reversed(request.bounds if request else ())):
        fields.extend(((14 + 4 * dimension, upper - lower + 1, 2), (16 + 4 * dimension, lower, 2)))
    return tuple(
        (mir.MemRef(Addr(Space.SEGMENT, start + offset, descriptor.index), width), mir.Const(number, width))
        for offset, number, width in fields
    )


def _addresses(body: mir.MirBody, symbols: dict[mir.Value, mir.Symbol]) -> mir.MirBody:
    """Resolve descriptor fields without moving their original relocation operands."""

    def reference(ref: mir.MemRef) -> mir.MemRef:
        symbol = symbols.get(ref.base)
        if symbol is None or symbol.space not in (Space.SEGMENT, Space.FRAME) or symbol.width != 2:
            return ref
        if ref.addr is None or ref.addr.space is not Space.LITERAL or ref.segment is not None:
            return ref
        offset = symbol.offset + symbol.addend + ref.addr.disp
        if (
            symbol.space is Space.SEGMENT
            and not 0 <= offset <= 0x10000 - ref.width
            or symbol.space is Space.FRAME
            and not -0x8000 <= offset <= 0x7FFF - ref.width + 1
        ):
            return ref
        return replace(ref, symbolic=mir.Symbol(symbol.space, symbol.index, offset, 2))

    def argument(arg: mir.Arg) -> mir.Arg:
        return mir.Cell(reference(arg.ref)) if isinstance(arg, mir.Cell) else arg

    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(
                    replace(
                        op,
                        loads=tuple(map(reference, op.loads)),
                        stores=tuple(map(reference, op.stores)),
                        args=tuple(map(argument, op.args)),
                        results=tuple(map(argument, op.results)),
                    )
                    for op in block.ops
                ),
            )
            for block in body.blocks
        ),
    )


def _argument(
    arg: mir.Arg, known: dict[mir.Value, consts.Known], symbols: dict[mir.Value, mir.Symbol]
) -> mir.Const | mir.Symbol | None:
    if isinstance(arg, (mir.Const, mir.Symbol)):
        return arg if arg.width == 2 else None
    if isinstance(arg, mir.FrameAddress) and arg.width == 2:
        # A local dynamic-array descriptor is still a symbolic object even
        # though its address is relative to this invocation's frame.  The
        # frame-address raise deliberately removed BP from MIR; preserve the
        # same descriptor identity here without putting the register back.
        return mir.Symbol(Space.FRAME, 0, arg.offset, arg.width)
    if not isinstance(arg, mir.Held) or arg.width != 2:
        return None
    if symbol := symbols.get(arg.value):
        return symbol
    value = known.get(arg.value)
    if value is None or value.width < arg.width:
        return None
    number = value.n & 0xFFFF
    return mir.Const(number if number < 0x8000 else number - 0x10000, 2)


def _shape(
    arguments: list[mir.Const | mir.Symbol | None],
) -> tuple[mir.Symbol, int, int, int] | None:
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
    return descriptor, width.n, count, dimensions.n >> 8


def _request(arguments: list[mir.Const | mir.Symbol | None], replaces: bool) -> mir.ArrayRequest | None:
    shape = _shape(arguments)
    if shape is None:
        return None
    descriptor, width, count, _attributes = shape
    bounds = []
    for index in range(count):
        lower, upper = arguments[index * 2 : index * 2 + 2]
        if not isinstance(lower, mir.Const) or not isinstance(upper, mir.Const) or upper.n < lower.n:
            return None
        bounds.append((lower.n, upper.n))
    return mir.ArrayRequest(descriptor, width, tuple(bounds), replaces)
