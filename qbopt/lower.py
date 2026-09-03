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


# What the machine calls each operation a pass can invent. Everything else
# keeps the operation it was raised with, which lowering reads off the node
# -- so this is only for the shapes MIR creates: a copy and a jump.
_MACHINE: dict[mir.Kind, tuple[ir.Operation, str]] = {
    mir.Kind.COPY: (ir.Operation.MOVE, "mov"),
    mir.Kind.JUMP: (ir.Operation.JUMP, "jmp"),
}



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
    # A value resolves to the register the original instruction had in the
    # same position where there is one. Emitting ir.Held instead hands the
    # choice to the allocation, and the allocation is applied to the ops it
    # re-encodes and not to the ones emitted from their own bytes -- so the
    # two disagree, and lngmix printed 110 for 142900 with the dividend
    # deleted. Where there is no such operand -- an operation a pass
    # invented -- ir.Held is the only honest answer and select resolves it.
    was_op, name = _MACHINE.get(op.kind, (op.op, op.name))
    return ir.Semantics(
        was_op,
        name,
        dests=tuple(_place(one, was.dests if was else (), i) for i, one in enumerate(op.results)),
        sources=tuple(_place(one, was.sources if was else (), i) for i, one in enumerate(op.args)),
        target=_target(op, was),
    )


def _place(arg: mir.Arg, had: tuple, index: int) -> ir.Loc:
    """One operand, keeping the register the instruction already had."""
    got = operand(arg)
    if not isinstance(got, ir.Held) or index >= len(had):
        return got
    was = had[index]
    return was if isinstance(was, ir.Reg) and was.width == got.width else got


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
