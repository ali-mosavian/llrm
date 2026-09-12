from dataclasses import replace

from iced_x86 import Register

from qbopt.model import ir


def native(node: ir.Node) -> ir.Node:
    if not isinstance(node, ir.Opaque) or node.semantics.op is not ir.Operation.RETURN:
        return node
    # Headerless C objects have no return type: retain both words of DX:AX.
    # SI/DI are observable too, including when the original leaf never used them.
    returned = tuple(ir.Reg(register, 2) for register in (Register.AX, Register.DX, Register.SI, Register.DI))
    uses = node.effects.uses
    return replace(
        node,
        effects=replace(
            node.effects, uses=None if uses is None else uses | {ir.ROOT[one.register] for one in returned}
        ),
        semantics=replace(node.semantics, sources=(*node.semantics.sources, *returned)),
    )
