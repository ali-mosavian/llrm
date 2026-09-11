"""Expose loaded address-space identities and their memory dependencies."""

from dataclasses import replace

from iced_x86 import Register

from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.objectfile.module import Space


def loaded(body: mir.MirBody) -> mir.MirBody:
    values = ssa.values(body)
    serial = max((value.id for value in values), default=0)
    variable = max((value.variable for value in values), default=0)
    origin = dict(body.origin)
    blocks = []
    for block in body.blocks:
        current = None
        ops = []
        for op in block.ops:
            if _selector(op):
                serial += 1
                variable += 1
                current = mir.Value(serial, op.at, variable=variable, version=1)
                origin[current] = Register.ES
                op = replace(op, defines=(current,), results=(mir.Held(current, 2),), merges={})
            elif current is not None:
                def reference(ref):
                    if ref.addr is not None and ref.addr.space is Space.FAR and ref.addr.segment == Register.ES:
                        return replace(ref, segment=current)
                    return ref

                def argument(arg):
                    return mir.Cell(reference(arg.ref)) if isinstance(arg, mir.Cell) else arg
                loads = tuple(map(reference, op.loads))
                stores = tuple(map(reference, op.stores))
                effects = getattr(op.node, "effects", None)
                reads = effects is None or effects.uses is None or Register.ES in effects.uses
                writes = effects is None or effects.defs is None or Register.ES in effects.defs
                if op.node is None and op.kind is mir.Kind.EXTRACT and not op.barrier:
                    reads = writes = False
                if reads or any(ref.segment == current for ref in loads + stores):
                    op = replace(op, uses=tuple(dict.fromkeys((*op.uses, current))))
                op = replace(op, loads=loads, stores=stores, args=tuple(map(argument, op.args)), results=tuple(map(argument, op.results)))
                if writes:
                    current = None
            ops.append(op)
        if current is not None and ops:
            ops[-1] = replace(ops[-1], uses=tuple(dict.fromkeys((*ops[-1].uses, current))))
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks), origin=origin)


def _selector(op: mir.Op) -> bool:
    if op.kind is not mir.Kind.LOAD or op.barrier or op.defines or op.stores:
        return False
    match op.args, op.results:
        case (mir.Cell(ref=ref),), (mir.Opaque(name="es"),):
            return (ref.width == 2 and op.loads == (ref,) and ref.addr is not None
                    and ref.addr.space is not Space.FAR and ref.addr.segment != Register.ES)
    return False
