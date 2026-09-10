"""Pack neighboring literal word stores after allocation."""

from dataclasses import replace

from iced_x86 import Register

from qbopt.model import ir, lir
from qbopt.objectfile.module import Space


def _plain(one):
    return not (one.clobbers or one.requires or one.delivers or one.defines or one.uses
                or one.spread or one.group is not None or one.symbol is True)


def _literal(one):
    if not _plain(one):
        return None
    match one.what:
        case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Mem() as cell,), (ir.Imm(number, 2, None),)):
            address = cell.addr
            if (cell.width == 2 and cell.base is None and cell.through == Register.NONE
                and cell.offset == 0 and address is not None and address.space is Space.SEGMENT
                and address.base == address.segment == Register.NONE and 0 <= address.disp <= 0xfffe):
                return cell, number
    return None


def combined(body: lir.LirBody) -> lir.LirBody:
    blocks = []
    nothing = ir.Semantics(ir.Operation.NOTHING, "", (), ())
    for block in body.blocks:
        insns = list(block.insns)
        pending = None
        for index, one in enumerate(insns):
            if one.what == nothing and _plain(one):
                continue
            current = _literal(one)
            if current is not None and pending is not None:
                previous, (low_cell, low) = pending
                high_cell, high = current
                if high_cell.addr == low_cell.addr.plus(2) and low_cell.addr.disp <= 0xfffc:
                    first = insns[previous]
                    what = replace(first.what, dests=(replace(low_cell, width=4),),
                                   sources=(ir.Imm((low & 0xffff) | ((high & 0xffff) << 16), 4),))
                    insns[previous] = replace(first, what=what, symbol=False)
                    insns[index] = replace(one, what=nothing, symbol=False)
                    pending = None
                    continue
            pending = (index, current) if current is not None else None
        blocks.append(replace(block, insns=tuple(insns)))
    return replace(body, blocks=tuple(blocks))
