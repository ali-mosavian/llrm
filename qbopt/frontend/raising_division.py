"""Recognize a sign-extended word dividend as one scalar signed division."""

from dataclasses import replace

from qbopt.model import ir, mir


def scalar(body: mir.MirBody) -> mir.MirBody:
    definitions = {value: op for block in body.blocks for op in block.ops for value in op.defines}

    def raised(op: mir.Op) -> mir.Op:
        if op.kind is not mir.Kind.DIV or op.name != "idiv" or len(op.args) != 3 or len(op.results) != 2:
            return op
        high, low, divisor = op.args
        if not all(isinstance(arg, mir.Held) and arg.width == 2 for arg in (*op.args, *op.results)):
            return op
        extension = definitions.get(high.value)
        if (
            extension is None or extension.op is not ir.Operation.EXTEND or extension.name != "cwd"
            or extension.args != (low,) or extension.results != (high,)
            or op.loads or op.stores or op.merges
        ):
            return op
        remaining = {low.value, divisor.value}
        return replace(
            op, kind=mir.Kind.DIVMOD, args=(low, divisor),
            uses=tuple(value for value in op.uses if value != high.value or value in remaining),
        )

    return replace(body, blocks=tuple(replace(block, ops=tuple(map(raised, block.ops))) for block in body.blocks))
