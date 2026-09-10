"""Name bounded indexed fields directly instead of retaining address arithmetic."""

from dataclasses import replace

from qbopt.model import mir
from qbopt.objectfile.module import Addr, Space


def named(body: mir.MirBody) -> mir.MirBody:
    definitions = {value: op for block in body.blocks for op in block.ops for value in op.defines}

    def parts(value, seen=frozenset()):
        if value in seen:
            return None
        op = definitions.get(value)
        if (op is None or op.results != (mir.Held(value, 2),) or op.loads or op.stores
            or op.barrier or op.merges):
            return None
        if op.kind is mir.Kind.COPY and len(op.args) == 1 and isinstance(op.args[0], mir.Held):
            return parts(op.args[0].value, seen | {value}) if op.args[0].width == 2 else None
        if op.kind is not mir.Kind.ADD or len(op.args) != 2:
            return None
        for base, offset in (op.args, op.args[::-1]):
            if not isinstance(base, mir.Held) or base.width != 2:
                continue
            if isinstance(offset, mir.Symbol) and offset.space is Space.SEGMENT and offset.width == 2:
                return offset, base.value, 0
            if isinstance(offset, mir.Const) and offset.width == 2:
                previous = parts(base.value, seen | {value})
                if previous is not None:
                    symbol, index, displacement = previous
                    return symbol, index, displacement + ((offset.n & 65535) ^ 32768) - 32768
        return None

    def reference(ref):
        if (ref.addr is None or ref.addr.space is not Space.LITERAL or ref.base is None
            or ref.base_width != 2 or ref.segment is not None or not ref.excludes):
            return ref
        expression = parts(ref.base)
        if expression is None:
            return ref
        symbol, index, displacement = expression
        displacement += symbol.offset + symbol.addend + ref.addr.disp
        if not 0 <= displacement <= 65536 - ref.width:
            return ref
        return replace(ref, addr=Addr(Space.SEGMENT, displacement, symbol.index, base=ref.addr.base),
                       base=index, symbolic=None)

    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            if op.kind not in (mir.Kind.LOAD, mir.Kind.STORE) or op.barrier or op.merges:
                ops.append(op)
                continue
            refs = {ref: reference(ref) for ref in (*op.loads, *op.stores)}
            if all(old == new for old, new in refs.items()):
                ops.append(op)
                continue
            def arg(one):
                return mir.Cell(refs[one.ref]) if isinstance(one, mir.Cell) and one.ref in refs else one
            args, results = tuple(map(arg, op.args)), tuple(map(arg, op.results))
            uses = tuple(dict.fromkeys([one.value for one in args if isinstance(one, mir.Held)]
                         + [value for ref in refs.values() for value in (ref.base, ref.segment) if value is not None]))
            ops.append(replace(op, args=args, results=results, uses=uses,
                loads=tuple(refs[ref] for ref in op.loads), stores=tuple(refs[ref] for ref in op.stores),
                node=None, made=None, raised=None, symbol=bool(op.stores)))
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))
