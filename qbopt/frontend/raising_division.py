"""Recognize a sign-extended word dividend as one scalar signed division."""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir


def scalar(body: mir.MirBody) -> mir.MirBody:
    definitions = {value: op for block in body.blocks for op in block.ops for value in op.defines}

    def raised(op: mir.Op) -> mir.Op:
        if op.kind is not mir.Kind.DIV or op.name != "idiv" or len(op.args) != 3 or len(op.results) != 2:
            return op
        high, low, divisor = op.args
        if not all(isinstance(arg, mir.Held) and arg.width == 2 for arg in (*op.args, *op.results)):
            return op
        if op.loads or op.stores or not _extends(high, low, definitions):
            return op
        # `idiv r16` ties dx:ax to the pair it writes. Dropping the high half
        # drops its tie with it: the lowering builds a fresh high from `cwd`,
        # so nothing carries the old dx into the remainder any more. A tie to
        # anything else is a shape this does not know.
        if not op.merges.keys() <= {high.value, low.value}:
            return op
        remaining = {low.value, divisor.value}
        return replace(
            op,
            kind=mir.Kind.DIVMOD,
            args=(low, divisor),
            merges={was: now for was, now in op.merges.items() if was != high.value},
            uses=tuple(value for value in op.uses if value != high.value or value in remaining),
        )

    return replace(body, blocks=tuple(replace(block, ops=tuple(map(raised, block.ops))) for block in body.blocks))


def _extends(high: mir.Held, low: mir.Held, definitions: dict) -> bool:
    """Whether `high` is the top word of the sign extension of `low`.

    `cwd` is BC's way of saying it, but not the only one: a dividend that
    reached a long by `movsx` is a whole dword, and the divide reads its
    halves through an extract. Both are one `idiv r16`.
    """
    extension = definitions.get(high.value)
    if extension is None:
        return False
    if (
        extension.op is ir.Operation.EXTEND
        and extension.name == "cwd"
        and extension.args == (low,)
        and extension.results == (high,)
    ):
        return True
    if (
        extension.kind is not mir.Kind.EXTRACT
        or extension.results != (high,)
        or len(extension.args) != 2
        or extension.loads
        or extension.stores
        or extension.barrier
        or not isinstance(extension.args[0], mir.Held)
        or extension.args[0].width != 4
        or not isinstance(extension.args[1], mir.Const)
        or extension.args[1].n != 16
    ):
        return False
    whole = definitions.get(extension.args[0].value)
    return (
        whole is not None
        and whole.kind is mir.Kind.SIGN_EXTEND
        and whole.args == (low,)
        and whole.results == (extension.args[0],)
        and not whole.loads
        and not whole.stores
        and not whole.barrier
    )
