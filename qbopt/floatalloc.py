"""Place self-contained floating value chains on the target register stack."""

from dataclasses import replace

from qbopt import ir, mir


def placed(block: mir.MirBlock) -> mir.MirBlock:
    """Assign self-contained value chains to slots in evaluation order."""
    from qbopt.lower import Unlowered

    stack: list[int] = []
    ops = []
    for op in block.ops:
        origin = op.floating_origin
        if origin is None:
            if stack and (op.barrier or op.kind is mir.Kind.CALL or op.stack is not None):
                raise Unlowered("floating stack crosses an unmodelled operation")
            ops.append(op)
            continue
        for current, before in ((op.args, origin.inputs), (op.results, origin.outputs)):
            if len(current) != len(before) or any(type(arg) is not type(old)
                or (isinstance(old, mir.Held) and old.width == 10 and arg.width != 10)
                for arg, old in zip(current, before)):
                raise Unlowered("floating operand conversion requires instruction selection")

        def source(arg):
            if not isinstance(arg, mir.Held) or arg.width != 10:
                return arg
            if arg.value.variable not in stack:
                raise Unlowered("floating stack input is unavailable")
            return mir.Opaque(None, f"st{stack.index(arg.value.variable)}")

        inputs = tuple(map(source, op.args))
        match op.op:
            case ir.Operation.FLOAT_LOAD:
                delta, slot = 1, 0
            case ir.Operation.FLOAT_STORE:
                delta, slot = -1, None
                if not inputs or inputs[0] != mir.Opaque(None, "st0"):
                    raise Unlowered("floating stack store requires an exchange")
            case ir.Operation.FLOAT_ARITH | ir.Operation.FLOAT_UNARY:
                delta, slot = 0, 0
                if not inputs or inputs[0] != mir.Opaque(None, "st0"):
                    raise Unlowered("floating stack arithmetic requires an exchange")
            case ir.Operation.FLOAT_ARITH_POP:
                delta = -1
                if len(inputs) != 2 or inputs[1] != mir.Opaque(None, "st0"):
                    raise Unlowered("floating stack popping arithmetic requires an exchange")
                slot = stack.index(op.args[0].value.variable)
            case _:
                raise Unlowered("floating stack operation has no allocation rule")
        if op.stack != delta:
            raise Unlowered("floating stack transition disagrees with the instruction")
        outputs = []
        for arg in op.results:
            if not isinstance(arg, mir.Held) or arg.width != 10:
                outputs.append(arg)
                continue
            if slot is None or arg.value.variable in stack:
                raise Unlowered("floating stack result is not a fresh value")
            outputs.append(mir.Opaque(None, f"st{slot}"))
            if delta == 1:
                if len(stack) == 8:
                    raise Unlowered("floating stack requires a spill")
                stack.insert(0, arg.value.variable)
            else:
                if slot >= len(stack):
                    raise Unlowered("floating stack result has no slot")
                stack[slot] = arg.value.variable
        if delta == -1:
            if not stack:
                raise Unlowered("floating stack pop has no value")
            stack.pop(0)
        placed = replace(origin, inputs=op.args, outputs=op.results,
                         machine_inputs=inputs, machine_outputs=tuple(outputs))
        ops.append(replace(op, floating_origin=placed))
    if stack:
        raise Unlowered("floating stack live-out requires allocation")
    return replace(block, ops=tuple(ops))

