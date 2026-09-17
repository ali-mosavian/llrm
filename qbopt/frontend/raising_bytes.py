"""Recognize BC's high-byte clearing as a whole-value mask."""

from dataclasses import replace

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import mir


def scalar(body: mir.MirBody) -> mir.MirBody:
    # Read by an operation, or by a phi something reads: every loop header
    # has a flags phi, and counting its unread incoming edges kept oimad's
    # `xor bh,bh` before an OUT as a register MIR cannot name.
    observed = {value for block in body.blocks for op in block.ops for value in op.uses}
    phis = [phi for block in body.blocks for phi in block.phis]
    while grown := {value for phi in phis if phi.result in observed for value in phi.incoming.values()} - observed:
        observed |= grown

    def raised(op):
        if op.kind is not mir.Kind.XOR or len(op.args) != 2 or op.args[0] != op.args[1]:
            return op
        match op.args[0]:
            case mir.Opaque(what=ir.Reg(register=register, width=1)) if register in (
                Register.AH,
                Register.BH,
                Register.CH,
                Register.DH,
            ):
                pass
            case _:
                return op
        # Its flags are XOR's, not AND's, so the rewrite needs nobody to read them.
        if op.loads or op.stores or op.barrier or any(v.flags and v in observed for v in op.defines):
            return op
        root = ir.ROOT[register]
        inputs = [v for v in op.uses if body.origin.get(v) == root]
        outputs = [v for v in op.defines if body.origin.get(v) == root]
        if len(inputs) != 1 or len(outputs) != 1:
            return op
        before, after = inputs[0], outputs[0]
        return mir.detached(
            op,
            kind=mir.Kind.AND,
            name="and",
            args=(mir.Held(before, 2), mir.Const(255, 2)),
            results=(mir.Held(after, 2),),
            defines=(after,),
            uses=(before,),
            merges={before: after},
            raised=None,
        )

    return replace(body, blocks=tuple(replace(block, ops=tuple(map(raised, block.ops))) for block in body.blocks))
