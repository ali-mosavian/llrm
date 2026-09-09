"""Check the floating identity baseline before restoring stack operands."""

from dataclasses import replace

from qbopt import mir


def operation(op: mir.Op) -> mir.Op:
    from qbopt.lower import Unlowered

    origin = op.floating_origin
    if origin is None:
        return op
    if op.kind != origin.kind or op.floating != origin.semantics:
        raise Unlowered("floating evaluation semantics changed before stack allocation is implemented")
    variables = {arg.value.variable for arg in (*origin.inputs, *origin.outputs)
                 if isinstance(arg, mir.Held) and arg.width == 10}

    def operands(current, before, machine):
        if len(current) != len(before) or len(before) != len(machine):
            raise Unlowered("floating operand arity changed")
        out = []
        for arg, original, old in zip(current, before, machine):
            if isinstance(original, mir.Held) and original.width == 10:
                if (not isinstance(arg, mir.Held) or arg.width != 10
                    or arg.value.variable != original.value.variable):
                    raise Unlowered("floating dataflow changed before stack allocation is implemented")
                out.append(old)
            else:
                if type(arg) is not type(original):
                    raise Unlowered("floating input conversion changed")
                out.append(arg)
        return tuple(out)
    return replace(op,
        args=operands(op.args, origin.inputs, origin.machine_inputs),
        results=operands(op.results, origin.outputs, origin.machine_outputs),
        uses=tuple(value for value in op.uses if value.variable not in variables),
        defines=tuple(value for value in op.defines if value.variable not in variables),
        floating_origin=None)


def restored(body: mir.MirBody) -> mir.MirBody:
    from qbopt.lower import Unlowered

    floating = {arg.value.variable for block in body.blocks for op in block.ops if op.floating_origin is not None
                for arg in (*op.floating_origin.inputs, *op.floating_origin.outputs)
                if isinstance(arg, mir.Held) and arg.width == 10}
    for block in body.blocks:
        if any(value.variable in floating for phi in block.phis for value in (phi.result, *phi.incoming.values())):
            raise Unlowered("floating phi requires stack allocation")
        if any(op.floating_origin is None and any(value.variable in floating for value in (*op.uses, *op.defines))
               for op in block.ops):
            raise Unlowered("floating value used outside its original computation")
    blocks = []
    for block in body.blocks:
        typed = [op for op in block.ops if op.floating_origin is not None and op.kind is not mir.Kind.NOTHING]
        if not typed:
            blocks.append(block)
            continue
        baseline = typed[0].floating_origin
        sequence = tuple(op.floating_origin.at for op in typed)
        if block.at != baseline.block or sequence != baseline.sequence:
            raise Unlowered("floating sequence changed before stack allocation is implemented")
        blocks.append(replace(block, ops=tuple(operation(op) for op in block.ops)))
    return replace(body, blocks=tuple(blocks))
