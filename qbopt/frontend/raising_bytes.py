"""Recognize BC's high-byte clearing as a whole-value mask."""
from dataclasses import replace

from iced_x86 import Register

from qbopt.model import ir, mir


def scalar(body: mir.MirBody) -> mir.MirBody:
    observed = {value for block in body.blocks for op in block.ops for value in op.uses}
    observed |= {value for block in body.blocks for phi in block.phis for value in phi.incoming.values()}

    def raised(op, overwritten):
        if op.kind is not mir.Kind.XOR or len(op.args) != 2 or op.args[0] != op.args[1]:
            return op
        match op.args[0]:
            case mir.Opaque(what=ir.Reg(register=register, width=1)) if register in (
                Register.AH, Register.BH, Register.CH, Register.DH
            ):
                pass
            case _:
                return op
        if (not overwritten or op.loads or op.stores or op.barrier
            or any(v.flags and v in observed for v in op.defines)):
            return op
        root = ir.ROOT[register]
        inputs = [v for v in op.uses if body.origin.get(v) == root]
        outputs = [v for v in op.defines if body.origin.get(v) == root]
        if len(inputs) != 1 or len(outputs) != 1:
            return op
        before, after = inputs[0], outputs[0]
        return replace(op, kind=mir.Kind.AND, name="and", args=(mir.Held(before, 2), mir.Const(255, 2)),
                       results=(mir.Held(after, 2),), defines=(after,), uses=(before,),
                       merges={before: after}, node=None, made=None, raised=None)

    blocks = []
    for block in body.blocks:
        overwritten = False
        ops = []
        for op in reversed(block.ops):
            ops.append(raised(op, overwritten))
            overwritten |= any(value.flags for value in op.defines)
        blocks.append(replace(block, ops=tuple(reversed(ops))))
    return replace(body, blocks=tuple(blocks))
