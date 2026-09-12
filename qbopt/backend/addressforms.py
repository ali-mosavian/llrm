from dataclasses import replace

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
        if not isinstance(arg, ir.Mem) or arg.base is None or arg.base.width != 2:
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
