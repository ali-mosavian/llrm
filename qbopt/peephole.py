"""Simplifications that depend on the final physical register assignment."""

from dataclasses import replace

from iced_x86 import RegisterExt

from qbopt import ir, lir, target
from qbopt.passes import LIRTransform


class Peephole(LIRTransform):
    name = "peephole"

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return constants(body)


def constants(body: lir.LirBody) -> lir.LirBody:
    blocks = []
    for block in body.blocks:
        held = {}
        redundant = set()
        for one in block.insns:
            what = one.what
            if what is None or what.op is not ir.Operation.MOVE or what.name != "mov":
                held.clear()
                continue
            candidate = None
            if len(what.dests) == len(what.sources) == 1:
                dest, source = what.dests[0], what.sources[0]
                if (isinstance(dest, ir.Reg) and dest.register in target.WIDTHS
                    and isinstance(source, ir.Imm) and source.address is None
                    and dest.width == source.width):
                    candidate = dest, source.value & ((1 << (dest.width * 8)) - 1)
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
