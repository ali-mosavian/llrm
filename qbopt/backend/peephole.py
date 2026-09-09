"""Simplifications that depend on the final physical register assignment."""

from dataclasses import replace

from iced_x86 import RegisterExt

from qbopt.model import ir, lir
from qbopt.backend import target
from qbopt.model.passes import LIRTransform


class Peephole(LIRTransform):
    name = "peephole"

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return waits(constants(body))


_WAITING = frozenset({
    "wait", "fwait", "fld", "fild", "fst", "fstp", "fist", "fistp",
    "fadd", "faddp", "fiadd", "fsub", "fsubp", "fsubr", "fsubrp", "fisub", "fisubr",
    "fmul", "fmulp", "fimul", "fdiv", "fdivp", "fdivr", "fdivrp", "fidiv", "fidivr",
    "fchs", "fabs", "fsqrt", "fxch", "fcom", "fcomp", "fcompp", "fucom", "fucomp", "fucompp",
})


def waits(body: lir.LirBody) -> lir.LirBody:
    """An immediately following waiting instruction already checks pending FP exceptions.

    Intel SDM Vol. 1 section 8.3.12. Never cross integer work, an unknown
    instruction, a non-waiting control instruction, or a block boundary.
    """
    blocks = []
    for block in body.blocks:
        following = None
        redundant = set()
        for one in reversed(block.insns):
            what = one.what
            if what is not None and what.op is ir.Operation.NOTHING and not what.name:
                continue
            if (what is not None and what.name in {"wait", "fwait"}
                and following is not None and following.name in _WAITING):
                redundant.add(id(one))
            else:
                following = what
        blocks.append(replace(block, insns=tuple(lir.without(block.insns, lambda one: id(one) in redundant))))
    return replace(body, blocks=tuple(blocks))


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
