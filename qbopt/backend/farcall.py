"""Give a far indirect call the memory operand the 16-bit ISA requires.

Near indirect calls may name a word register.  A 16:16 far call has only an
``m16:16`` form, while MIR quite properly carries the code pointer as one
machine-neutral four-byte value.  Lower that target through one reusable owned
frame cell before allocation; this is target form selection, not spilling.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import frame as frames
from qbopt.model.passes import LIRTransform


class FarIndirectCalls(LIRTransform):
    name = "far-indirect-calls"

    def __init__(self, frame: frames.Frame) -> None:
        self.frame = frame

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return materialized(body, self.frame)


def materialized(body: lir.LirBody, frame: frames.Frame) -> lir.LirBody:
    """Materialize every packed far call target in one shared frame slot."""
    slot = None
    blocks = []
    for block in body.blocks:
        insns = []
        for one in block.insns:
            match one.what:
                case ir.Semantics(
                    op=ir.Operation.CALL,
                    indirect=True,
                    sources=(ir.Held(value=value, width=4) as target,),
                ):
                    if slot is None:
                        slot = frame.cell(("far-indirect-call", 0), 4)
                    insns.append(
                        lir.Insn(
                            one.at,
                            (one.at, one.at),
                            ir.Semantics(ir.Operation.MOVE, "mov", (slot,), (target,)),
                            (),
                            (value,),
                            symbol=False,
                        )
                    )
                    insns.append(replace(one, what=replace(one.what, sources=(slot,)), uses=()))
                case _:
                    insns.append(one)
        blocks.append(replace(block, insns=tuple(insns)))
    return replace(body, blocks=tuple(blocks))
