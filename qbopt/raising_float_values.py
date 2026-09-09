"""Resolve self-contained floating stacks to ordinary MIR value operands."""

from dataclasses import replace

from qbopt import fpstack, mir, ssa


def raised(body: mir.MirBody) -> mir.MirBody:
    values = tuple(ssa.values(body))
    serial = max((value.id for value in values), default=0)
    variable = max((value.variable for value in values), default=0)
    readings = fpstack.readings(body)
    blocks = []
    for block in body.blocks:
        operations = [op for op in block.ops if op.stack is not None]
        if (not operations or any(op.floating is None for op in operations)
            or len({op.at for op in operations}) != len(operations)
            or sum(op.stack for op in operations) != 0):
            blocks.append(block)
            continue
        known = set()
        valid = True
        for op in operations:
            reading = readings.get(op.at)
            if reading is None or any(value not in known for value in reading.uses.values()):
                valid = False
                break
            if any(isinstance(arg, mir.Opaque) for arg in op.results) and reading.defines is None:
                valid = False
                break
            if reading.defines is not None:
                known.add(reading.defines)
        if not valid:
            blocks.append(block)
            continue
        held = {}
        for value in sorted(known, key=lambda one: one.id):
            serial += 1
            variable += 1
            held[value] = mir.Held(mir.Value(serial, value.at, variable=variable, version=1), 10)
        sequence = tuple(op.at for op in operations)
        selected = {id(op) for op in operations}
        ops = []
        for op in block.ops:
            if id(op) not in selected:
                ops.append(op)
                continue
            reading = readings[op.at]
            def source(arg):
                if isinstance(arg, mir.Opaque) and arg.name.startswith("st") and arg.name[2:].isdigit():
                    return held[reading.uses[int(arg.name[2:])]]
                return arg
            args = tuple(source(arg) for arg in op.args)
            results = tuple(held[reading.defines] if isinstance(arg, mir.Opaque) else arg for arg in op.results)
            baseline = mir.FloatingOrigin(block.at, sequence, op.at, op.kind, op.floating,
                                          args, results, op.args, op.results)
            uses = tuple(dict.fromkeys((*op.uses, *(arg.value for arg in args if isinstance(arg, mir.Held)))))
            defines = tuple(dict.fromkeys((*op.defines, *(arg.value for arg in results if isinstance(arg, mir.Held)))))
            ops.append(replace(op, args=args, results=results, uses=uses, defines=defines, floating_origin=baseline))
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))
