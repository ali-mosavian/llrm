"""Separate scalar condition reads from their value comparisons."""

from dataclasses import replace

from qbopt.analysis import ssa
from qbopt.model import ir, mir
from qbopt.objectfile.module import Space


def loaded(body: mir.MirBody) -> mir.MirBody:
    values = tuple(ssa.values(body))
    serial = max((value.id for value in values), default=0)
    variable = max((value.variable for value in values), default=0)
    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            cells = [arg for arg in op.args if isinstance(arg, mir.Cell)]
            if (op.op is not ir.Operation.COMPARE or op.kind is not mir.Kind.SUB
                or op.results or len(op.defines) != 1 or not op.defines[0].flags
                or op.barrier or op.floating is not None or op.stack is not None or op.merges
                or op.stores or len(op.args) != 2 or len(cells) != 1
                or op.loads != (cells[0].ref,) or cells[0].ref.width not in (2, 4)
                or cells[0].ref.addr is None
                or cells[0].ref.addr.space not in (Space.SEGMENT, Space.FRAME, Space.LITERAL)
                or any(not isinstance(arg, (mir.Cell, mir.Held, mir.Const))
                       or (arg.ref.width if isinstance(arg, mir.Cell) else arg.width)
                       != cells[0].ref.width for arg in op.args)):
                ops.append(op)
                continue
            ref = cells[0].ref
            serial += 1
            variable += 1
            value = mir.Value(serial, op.at, variable=variable, version=1)
            held = mir.Held(value, ref.width)
            ops.append(mir.Op(op.at, ir.Operation.MOVE, "", (value,),
                tuple(part for part in (ref.base, ref.segment) if part is not None),
                kind=mir.Kind.LOAD, loads=(ref,), args=(cells[0],), results=(held,),
                id=next(mir._IDS), symbol=False))
            args = tuple(held if isinstance(arg, mir.Cell) else arg for arg in op.args)
            ops.append(mir.detached(op, args=args, loads=(),
                uses=tuple(dict.fromkeys(arg.value for arg in args if isinstance(arg, mir.Held))),
                raised=None))
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))
