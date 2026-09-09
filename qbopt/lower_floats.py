"""Validate floating value placement before restoring stack operands."""

from dataclasses import replace

from qbopt import floatalloc, mir




def _stack_checked(block: mir.MirBlock) -> None:
    from qbopt.lower import Unlowered

    stack: list[int] = []

    def slots(values, machine):
        if len(values) != len(machine):
            raise Unlowered("floating stack operand arity changed")
        for value, operand in zip(values, machine, strict=True):
            if isinstance(value, mir.Held) and value.width == 10:
                if (not isinstance(operand, mir.Opaque) or not operand.name.startswith("st")
                    or not operand.name[2:].isdigit()):
                    raise Unlowered("floating stack operand has no slot")
                yield value.value.variable, int(operand.name[2:])

    for op in block.ops:
        origin = op.floating_origin
        if origin is None:
            if stack and (op.barrier or op.kind is mir.Kind.CALL or op.stack is not None):
                raise Unlowered("floating stack crosses an unmodelled operation")
            continue
        if op.stack not in (-1, 0, 1):
            raise Unlowered("floating stack transition is unknown")
        for value, index in slots(op.args, origin.machine_inputs):
            if not 0 <= index < len(stack) or stack[index] != value:
                raise Unlowered("floating stack input does not hold the required value")
        outputs = list(slots(op.results, origin.machine_outputs))
        if op.stack == 1:
            if len(stack) == 8 or len(outputs) != 1 or outputs[0][1] != 0:
                raise Unlowered("floating stack push cannot be placed")
            stack.insert(0, outputs[0][0])
        else:
            for value, index in outputs:
                if not 0 <= index < len(stack):
                    raise Unlowered("floating stack result has no occupied slot")
                stack[index] = value
            if op.stack == -1:
                if not stack:
                    raise Unlowered("floating stack pop has no value")
                stack.pop(0)
    if stack:
        raise Unlowered("floating stack live-out requires allocation")


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
                for arg in (*op.args, *op.results)
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
        block = floatalloc.placed(block)
        _stack_checked(block)
        blocks.append(replace(block, ops=tuple(operation(op) for op in block.ops)))
    return replace(body, blocks=tuple(blocks))
