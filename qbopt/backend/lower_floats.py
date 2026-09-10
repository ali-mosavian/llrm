"""Validate floating rewrites and support the legacy per-operation baseline."""

from dataclasses import replace

from qbopt.model import mir


def _removed(op: mir.Op) -> bool:
    from qbopt.backend.lower import Unlowered

    if op.kind not in (mir.Kind.NOTHING, mir.Kind.FCHECK):
        return False
    if (op.args or op.results or op.uses or op.defines or op.loads or op.stores
        or op.floating is not None or op.stack is not None or op.node is not None
        or op.made is not None or op.raised is not None):
        raise Unlowered("removed floating operation retains computation")
    return True



def _stack_checked(block: mir.MirBlock) -> None:
    from qbopt.backend.lower import Unlowered

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
    from qbopt.backend.lower import Unlowered

    origin = op.floating_origin
    if origin is None:
        return op
    if _removed(op):
        return replace(op, floating_origin=None)
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


def _forwarded_inputs(op):
    from qbopt.model.floating import Format
    origin = op.floating_origin
    if (op.floating is None or origin.semantics is None
        or op.kind not in (mir.Kind.FADD, mir.Kind.FSUB, mir.Kind.FMUL, mir.Kind.FDIV)
        or len(op.args) != len(origin.inputs)
        or len(op.floating.inputs) != len(op.args)
        or replace(op.floating, inputs=origin.semantics.inputs) != origin.semantics):
        return set()
    return {index for index, (arg, old, format, before) in enumerate(zip(
        op.args, origin.inputs, op.floating.inputs, origin.semantics.inputs))
        if isinstance(old, mir.Cell) and isinstance(arg, mir.Held) and arg.width == 10
        and before in (Format.BINARY32, Format.BINARY64) and format is Format.EXTENDED80}


def checked(body: mir.MirBody) -> None:
    from qbopt.backend.lower import Unlowered

    repetitions = dict(body.repetitions)
    if (len(repetitions) != len(body.repetitions)
        or any(body.block(at) is None or not 2 <= count <= 4 for at, count in body.repetitions)):
        raise Unlowered("invalid block repetition provenance")

    floating = {arg.value.variable for block in body.blocks for op in block.ops if op.floating_origin is not None
                for arg in (*op.args, *op.results)
                if isinstance(arg, mir.Held) and arg.width == 10}
    for block in body.blocks:
        if any(value.variable in floating for phi in block.phis for value in (phi.result, *phi.incoming.values())):
            raise Unlowered("floating phi requires stack allocation")
        if any(op.floating_origin is None and any(value.variable in floating for value in (*op.uses, *op.defines))
               for op in block.ops):
            raise Unlowered("floating value used outside its original computation")
    for block in body.blocks:
        typed = [op for op in block.ops if op.floating_origin is not None]
        if not typed:
            continue
        baseline = typed[0].floating_origin
        sequence = tuple(op.floating_origin.at for op in typed)
        if block.at != baseline.block or sequence != baseline.sequence * repetitions.get(block.at, 1):
            raise Unlowered("floating sequence changed before stack allocation is implemented")
        for op in typed:
            if _removed(op):
                continue
            origin = op.floating_origin
            forwarded = _forwarded_inputs(op)
            semantics = op.floating
            if forwarded:
                formats = tuple(origin.semantics.inputs[index] if index in forwarded else format
                                for index, format in enumerate(semantics.inputs))
                semantics = replace(semantics, inputs=formats)
            if op.kind != origin.kind or semantics != origin.semantics:
                raise Unlowered("floating evaluation semantics changed")
            for current, before in ((op.args, origin.inputs), (op.results, origin.outputs)):
                allowed = forwarded if current is op.args else set()
                if len(current) != len(before) or any((type(arg) is not type(old) and index not in allowed)
                    or (isinstance(old, mir.Held) and old.width == 10 and arg.width != 10)
                    for index, (arg, old) in enumerate(zip(current, before))):
                    raise Unlowered("floating operand conversion requires instruction selection")
