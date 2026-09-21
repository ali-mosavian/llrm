"""Spell allocated address-valued memory operands as machine addresses."""

from dataclasses import replace

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.objectfile.module import Space


def converted(body: lir.LirBody) -> lir.LirBody:
    """Turn allocated ADDRESS cells into LEA's non-memory operand spelling."""

    def instruction(one: lir.Insn) -> lir.Insn:
        what = one.what
        if what is None or what.op is not ir.Operation.ADDRESS:
            return one
        sources = []
        for source in what.sources:
            if not isinstance(source, ir.Mem):
                sources.append(source)
                continue
            addr = source.addr
            if addr is not None and addr.space is Space.FRAME and source.base is not None:
                sources.append(
                    ir.Address(
                        None,
                        through=Register.BP,
                        index=source.through,
                        scale=source.scale,
                        offset=addr.disp,
                        disp_width=source.disp_width,
                    )
                )
                continue
            if addr is not None and source.base is not None and source.through != Register.NONE:
                addr = replace(addr, base=source.through)
            sources.append(
                ir.Address(
                    addr,
                    through=source.through,
                    index=source.index_through,
                    scale=source.scale,
                    offset=source.offset,
                    disp_width=source.disp_width,
                )
            )
        return replace(one, what=replace(what, sources=tuple(sources)))

    return replace(
        body,
        blocks=tuple(replace(block, insns=tuple(instruction(one) for one in block.insns)) for block in body.blocks),
    )
