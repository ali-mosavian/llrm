"""Simplifications that depend on the final physical register assignment."""

from dataclasses import replace

from iced_x86 import RegisterExt

from qbopt.model import ir, lir
from qbopt.backend import target
from qbopt.model.passes import LIRTransform


class Peephole(LIRTransform):
    name = "peephole"

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return constants(body)


def constants(body: lir.LirBody) -> lir.LirBody:
    """Reuse identical scalar register contents within a straight-line move sequence."""
    blocks = []
    for block in body.blocks:
        held = {}
        redundant = set()
        for one in block.insns:
            what = one.what
            move = what is not None and what.op is ir.Operation.MOVE and what.name == "mov"
            extend = (what is not None and what.op is ir.Operation.EXTEND
                      and what.name in {"cwd", "cdq", "movsx"}
                      and len(what.dests) == len(what.sources) == 1
                      and all(isinstance(arg, ir.Reg) for arg in (*what.dests, *what.sources)))
            if not move and not extend:
                held.clear()
                continue
            candidate = None
            if move and len(what.dests) == len(what.sources) == 1:
                dest, source = what.dests[0], what.sources[0]
                if (isinstance(dest, ir.Reg) and dest.register in target.WIDTHS
                    and isinstance(source, (ir.Reg, ir.Imm)) and dest.width == source.width):
                    if isinstance(source, ir.Imm) and source.address is None:
                        candidate = dest, source.value & ((1 << (dest.width * 8)) - 1)
                    elif isinstance(source, ir.Reg) and source.register in target.WIDTHS:
                        candidate = dest, held.setdefault(source, object())
            if candidate is not None and not one.clobbers and held.get(candidate[0]) == candidate[1]:
                redundant.add(id(one))
                continue
            written = {RegisterExt.full_register32(reg) for reg in one.clobbers}
            written.update(RegisterExt.full_register32(dest.register) for dest in what.dests if isinstance(dest, ir.Reg))
            held = {dest: value for dest, value in held.items()
                    if RegisterExt.full_register32(dest.register) not in written}
            if candidate is not None and not one.clobbers:
                held[candidate[0]] = candidate[1]
        blocks.append(replace(block, insns=tuple(lir.without(block.insns, lambda one: id(one) in redundant))))
    return replace(body, blocks=tuple(blocks))
