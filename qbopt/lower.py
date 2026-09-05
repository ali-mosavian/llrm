"""MIR to machine form.

The one place a value becomes a register operand. Everything above this is
MIR -- values, constants and cells -- and everything below is x86, which is
the boundary rule 5 draws.

What comes out names no register either: an ir.Held says *which value*, and
select.py resolves it through the allocation. Naming one here would only
move the pass's mistake down a layer.
"""

from dataclasses import replace

from iced_x86 import Register

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


class Unlowered(Exception):
    """An operand nothing here can turn into a machine location."""


def _located(what: "ir.Semantics | None", was: "ir.Semantics | None") -> "ir.Semantics | None":
    """`what` with every MIR operand in it replaced by a machine one.

    A cell is the one that needs help. mir.MemRef says which bytes and what
    its address depends on -- the alias question -- and ir.Mem says how to
    encode it: which register reaches it, and how wide the displacement
    field was, which is not how wide the number needs to be. Neither is
    derivable from the other, so the encoding half comes from the operand
    the same instruction had in the same position before a pass rewrote it.

    A cell in a position the original had none is an error rather than a
    guess. Encoding a displacement at the wrong width is `mov ax,[bx]`
    emitted as `8b 07` -- the right instruction reading the wrong address.
    """
    if what is None:
        return None
    dests = tuple(_machine(one, was.dests if was else (), index) for index, one in enumerate(what.dests))
    sources = tuple(_machine(one, was.sources if was else (), index) for index, one in enumerate(what.sources))
    if dests == what.dests and sources == what.sources:
        return what
    return ir.Semantics(what.op, what.name, dests, sources, what.target)


def _machine(one, had: tuple, index: int):
    if not isinstance(one, mir.MemRef):
        return one
    before = had[index] if index < len(had) else None
    if not isinstance(before, ir.Mem):
        before = next((x for x in had if isinstance(x, ir.Mem)), None)
    if before is not None:
        return ir.Mem(one.addr, one.width, before.through, before.offset, before.disp_width)
    return _addressed(one)


def _addressed(one: "mir.MemRef") -> "ir.Mem":
    """A cell the original instruction had no memory operand for.

    A pass put it there -- a fold that turned a register read back into the
    read of the cell it came from -- so there is no encoding to copy and it
    has to come from the address itself. Only for the two spaces whose
    encoding the address fully determines: a frame slot is reached through
    bp and a segment-relative cell through no register at all, both with a
    two-byte displacement, which is what BC emits and what a fixup expects.

    Anything else -- a far pointer, an indexed element, the stack -- is
    refused by name. Guessing which register reaches it would be guessing
    the instruction.
    """
    from qbopt.module import Space

    addr = one.addr
    if addr is None:
        raise Unlowered("a cell with no address cannot be encoded: nothing says which register reaches it")
    if addr.space is Space.FRAME:
        return ir.Mem(addr, one.width, Register.BP, 0, 2)
    if addr.space is Space.SEGMENT and addr.base == Register.NONE:
        return ir.Mem(addr, one.width, Register.NONE, 0, 2)
    raise Unlowered(f"a cell at {addr} in {addr.space} has no encoding this can derive")


def lowered(name: str, body: "mir.MirBody") -> "lir.LirBody":
    """One MIR body as machine instructions, and nothing else.

    The pass that ends the abstract half. Above this a value is a value and
    an operation says what it computes; below it every operand is a
    location and the only questions left are which register, where the
    bytes go and what a fixup names.

    One instruction per operation, in the order the blocks give. `what` is
    None where nothing rewrote the operation, which is layout's signal to
    carry the original bytes rather than re-encode them -- a re-encode that
    lands on a longer form for the same instruction is how a rebuild grows
    without anything having been optimised.
    """
    from qbopt import lir

    return lir.LirBody(
        name=name,
        entry=body.entry,
        blocks=tuple(
            lir.LirBlock(
                at=block.at,
                insns=tuple(
                    lir.Insn(
                        at=op.at,
                        covers=op.covers,
                        what=_located(current(op), getattr(op.node, "semantics", None)),
                        defines=tuple(one.id for one in op.defines if not one.flags),
                        uses=tuple(one.id for one in op.uses if not one.flags),
                        op=op,
                    )
                    for op in block.ops
                ),
                succ=block.succ,
                arrives=tuple(phi.result.id for phi in block.phis if not phi.result.flags),
            )
            for block in body.blocks
        ),
        origin=dict(body.origin),
        pins=dict(getattr(body, "pins", {}) or {}),
    )
