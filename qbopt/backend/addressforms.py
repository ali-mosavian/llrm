from dataclasses import replace

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import mir
from qbopt.objectfile.module import Space


def offsets(body: mir.MirBody) -> dict[int, tuple[ir.Held, int]]:
    result = {}
    for block in body.blocks:
        for op in block.ops:
            if op.loads or op.stores or op.barrier:
                continue
            match op.kind, op.args, op.results:
                case mir.Kind.COPY, (mir.Held(width=2) as source,), (mir.Held(value=dest, width=2),):
                    result[dest.id] = ir.Held(source.value.id, 2), 0
                case mir.Kind.ADD, (mir.Held(width=2) as source, mir.Const(n=amount, width=2)), (
                    mir.Held(value=dest, width=2),
                ):
                    result[dest.id] = ir.Held(source.value.id, 2), amount
                case _:
                    pass
    return result


def selected(what: ir.Semantics | None, forms: dict[int, tuple[ir.Held, int]]) -> ir.Semantics | None:
    if what is None:
        return None

    def operand(arg: object) -> object:
        if not isinstance(arg, ir.Mem) or arg.base is None or arg.base.width != 2 or arg.index is not None:
            return arg
        if arg.addr is None or arg.addr.space is not Space.FAR:
            return arg
        base, offset = arg.base, 0
        seen = set()
        while base.value in forms and base.value not in seen:
            seen.add(base.value)
            base, step = forms[base.value]
            offset += step
        if base.value in seen:
            return arg
        displacement = (arg.offset + offset + 32768) % 65536 - 32768
        return (
            replace(
                arg,
                base=base,
                offset=displacement,
                disp_width=2,
                addr=replace(arg.addr, disp=(arg.addr.disp + offset + 32768) % 65536 - 32768),
            )
            if seen
            else arg
        )

    return replace(what, dests=tuple(map(operand, what.dests)), sources=tuple(map(operand, what.sources)))


# Scales 32-bit addressing encodes; 16-bit `[bx+si]` has none but one.
_SCALES = {4: (0, 1, 2, 3), 2: (0,)}


def indexed(body: mir.MirBody, exposed: set[int]) -> tuple[dict[int, tuple[ir.Held, ir.Held, int]], frozenset[int]]:
    """Based addresses `b + (c << k)` read only by cells, and what computes them.

    The address becomes the cell's `[base+index*scale]` and the add and
    shift that computed it become nothing. Only where no flag they set is
    read and nothing but an encodable cell's base reads the address.  This
    applies equally to far pointers and near pointers into local or global
    objects; the address width below decides whether a scale is legal.
    """
    made = {value.id: op for block in body.blocks for op in block.ops for value in op.defines}
    bases: dict[int, int] = {}
    other: dict[int, int] = {}
    for block in body.blocks:
        for phi in block.phis:
            for value in phi.incoming.values():
                other[value.id] = other.get(value.id, 0) + 1
        for op in block.ops:
            based = {
                one.ref.base.id
                for one in (*op.args, *op.results)
                if isinstance(one, mir.Cell)
                and one.ref.base is not None
                and one.ref.addr is not None
                and one.ref.where not in (Space.GROUP, Space.STACK)
            }
            held = [one.value.id for one in op.args if isinstance(one, mir.Held)]
            for value in op.uses:
                if value.id in based and value.id not in held:
                    bases[value.id] = bases.get(value.id, 0) + 1
                else:
                    other[value.id] = other.get(value.id, 0) + 1
    for value in exposed:
        other[value] = other.get(value, 0) + 1

    def plain(op: mir.Op, kind: mir.Kind) -> bool:
        return (
            op.kind is kind
            and not (op.loads or op.stores or op.merges or op.barrier)
            and len(op.results) == 1
            and isinstance(op.results[0], mir.Held)
            and not any(one.flags and (one.id in other or one.id in bases) for one in op.defines)
        )

    forms: dict[int, tuple[ir.Held, ir.Held, int]] = {}
    folded: set[int] = set()
    for block in body.blocks:
        for op in block.ops:
            if not plain(op, mir.Kind.ADD) or len(op.args) != 2:
                continue
            address = op.results[0]
            if address.value.id in other or address.value.id not in bases or address.width not in _SCALES:
                continue
            if not all(isinstance(one, mir.Held) and one.width == address.width for one in op.args):
                continue
            base, index = op.args
            form = (ir.Held(base.value.id, base.width), ir.Held(index.value.id, index.width), 1)
            for base, index in (op.args, op.args[::-1]):
                shift = made.get(index.value.id)
                if (
                    shift is not None
                    and plain(shift, mir.Kind.SHL)
                    and len(shift.args) == 2
                    and isinstance(shift.args[0], mir.Held)
                    and shift.args[0].width == address.width
                    and isinstance(shift.args[1], mir.Const)
                    and shift.args[1].n in _SCALES[address.width]
                    and other.get(index.value.id, 0) == 1
                    and index.value.id not in bases
                ):
                    counter = shift.args[0]
                    form = (ir.Held(base.value.id, base.width), ir.Held(counter.value.id, counter.width), 1 << shift.args[1].n)
                    folded.add(index.value.id)
                    break
            forms[address.value.id] = form
            folded.add(address.value.id)
    return forms, frozenset(folded)


def scaled(what: ir.Semantics | None, forms: dict[int, tuple[ir.Held, ir.Held, int]]) -> ir.Semantics | None:
    """`what` with every folded far address written as its cell's base and index."""
    if what is None or not forms:
        return what

    def operand(arg: object) -> object:
        if not isinstance(arg, ir.Mem) or arg.base is None or arg.base.value not in forms:
            return arg
        base, index, scale = forms[arg.base.value]
        return replace(arg, base=base, index=index, scale=scale, through=Register.NONE)

    return replace(what, dests=tuple(map(operand, what.dests)), sources=tuple(map(operand, what.sources)))
