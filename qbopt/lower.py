"""MIR to machine form.

The one place a value becomes a register operand. Everything above this is
MIR -- values, constants and cells -- and everything below is x86, which is
the boundary rule 5 draws.

What comes out names no register either: an ir.Held says *which value*, and
select.py resolves it through the allocation. Naming one here would only
move the pass's mistake down a layer.
"""

from dataclasses import replace

from qbopt import ir
from qbopt import mir


def operand(arg: mir.Arg) -> ir.Loc:
    """One MIR operand as the machine's."""
    if isinstance(arg, mir.Held):
        return ir.Held(value=arg.value.id, width=arg.width)
    if isinstance(arg, mir.Const):
        return ir.Imm(value=arg.n, width=arg.width)
    if isinstance(arg, mir.Cell):
        return arg.ref
    return arg.what  # the x87 stack, which has no MIR form


def semantics(op: mir.Op, was: ir.Semantics | None = None) -> ir.Semantics | None:
    """What this operation computes, in machine form, or None for verbatim.

    None means nothing rewrote it: `was` is still what it says, and layout
    emits the original bytes rather than re-encoding them. A re-encode that
    lands on a longer form for the same instruction is how a rebuild starts
    growing without anything having been optimised.
    """
    same_target = was is None or op.target == was.target
    if op.raised is not None and (op.args, op.results) == op.raised and same_target:
        return None  # nothing rewrote it
    if not op.args and not op.results and op.raised is None:
        return None  # nothing to build one from
    return ir.Semantics(
        op.op,
        op.name,
        dests=tuple(operand(one) for one in op.results),
        sources=tuple(operand(one) for one in op.args),
        target=_target(op, was),
    )


def current(op) -> "ir.Semantics | None":
    """What this operation computes now, in machine form.

    The one answer to the question every consumer used to ask as
    `op.made if op.made is not None else op.node.semantics` -- which read
    the *original* instruction for an operation a pass had rewritten in
    MIR's own operands, and so told twenty callers the fold had not
    happened.
    """
    was = getattr(op.node, "semantics", None)
    if op.made is not None:
        return op.made
    return semantics(op, was) or was


def _target(op: mir.Op, was: ir.Semantics | None) -> int | None:
    """A branch's destination, which is a block address and not an operand."""
    if op.target is not None:
        return op.target
    return was.target if was is not None else None


__all__ = ["operand", "semantics", "current"]
