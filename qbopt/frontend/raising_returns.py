from dataclasses import replace

from iced_x86 import Register

from qbopt.model import ir


def returned(node: ir.Node, header: bool, registers: tuple | None = None) -> ir.Node:
    """A return, reading the registers its caller reads after it.

    BC's FUNCTION answers in AX or DX:AX.  ``registers`` is the procedure's
    CodeView-established return class; a SUB and an implicit-INTEGER function
    remain indistinguishable, but neither returns DX.  Missing debug evidence
    keeps the conservative DX:AX default.  A headerless C object has no return
    type, so both words of DX:AX, and SI/DI too, including when the original
    leaf never used them.
    """
    if not isinstance(node, ir.Opaque) or node.semantics.op is not ir.Operation.RETURN:
        return node
    registers = registers or (
        (Register.AX, Register.DX) if header else (Register.AX, Register.DX, Register.SI, Register.DI)
    )
    returned = tuple(ir.Reg(register, 2) for register in registers)
    uses = node.effects.uses
    return replace(
        node,
        effects=replace(
            node.effects, uses=None if uses is None else uses | {ir.ROOT[one.register] for one in returned}
        ),
        semantics=replace(node.semantics, sources=(*node.semantics.sources, *returned)),
    )
