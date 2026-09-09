"""Recognize runtime floating-to-integer conversions at the machine boundary."""

from dataclasses import replace

from iced_x86 import Register

from qbopt.abi import runtime
from qbopt.analysis import ssa
from qbopt.model import ir, mir
from qbopt.objectfile import module, omf


def raised(body, found, contracts):
    if "FIDRQQ" not in omf.externals(found.records):
        return body
    local = module.defines(found.records, found.seg)
    values = tuple(ssa.values(body))
    serial = max((value.id for value in values), default=0)
    variable = max((value.variable for value in values), default=0)
    used = {value for block in body.blocks for op in block.ops for value in op.uses}
    used.update(value for block in body.blocks for phi in block.phis for value in phi.incoming.values())
    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            name = found.calls.get(op.at)
            width = {"B$FIS2": 2, "B$FIST": 4}.get(name)
            rule = contracts.get(op.at)
            expected = runtime.contract(name) if width else None
            outputs = {body.origin.get(value): value for value in op.defines if not value.flags}
            registers = (Register.EAX,) if width == 2 else (Register.EAX, Register.EDX)
            if (width is None or name in local or op.kind is not mir.Kind.CALL
                or rule is None or not rule.established or rule != expected
                or op.merges or op.args or any(value.flags and value in used for value in op.defines)
                or set(outputs) != set(registers)):
                ops.append(op)
                continue
            serial += 1
            variable += 1
            result = mir.Held(mir.Value(serial, op.at, variable=variable, version=1), width)
            converted = replace(op, kind=mir.Kind.FSTORE, op=ir.Operation.FLOAT_STORE, name="fistp",
                args=(mir.Opaque(ir.St(0), "st0"),), results=(result,), defines=(result.value,),
                uses=(), loads=(), stores=(), merges={}, node=None, made=None, raised=None,
                stack=-1, symbol=False)
            ops.append(converted)
            found.float_protocols[op.id] = 0x34
            for shift, register in enumerate(registers):
                target = outputs[register]
                ops.append(mir.Op(op.at, ir.Operation.MOVE, "", (target,), (result.value,),
                    kind=mir.Kind.EXTRACT, args=(result, mir.Const(16 * shift, 4)),
                    results=(mir.Held(target, 2),), covers=(op.at, op.at), symbol=False))
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))
