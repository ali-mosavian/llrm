"""Recognize integer-to-floating helpers as conversions of ordinary MIR values."""

from dataclasses import replace

from iced_x86 import Register

from qbopt.abi import runtime
from qbopt.model import ir, mir
from qbopt.objectfile import module, omf


_WIDTHS = {"B$FILD": 4, "B$FIL2": 2}


def _source(op, width, origin, definitions):
    inputs = {origin.get(arg.value): arg
              for arg in op.args if isinstance(arg, mir.Held)}
    argument = inputs.get(Register.EAX)
    if width == 2:
        return argument
    low, high = (definitions.get(arg.value) if arg is not None else None
                 for arg in (argument, inputs.get(Register.EDX)))
    if (low is not None and high is not None
        and low[1].kind is mir.Kind.EXTRACT and high[1].kind is mir.Kind.EXTRACT
        and len(low[1].args) == len(high[1].args) == 2
        and low[1].args[1] == mir.Const(0, 4) and high[1].args[1] == mir.Const(16, 4)
        and low[1].args[0] == high[1].args[0] and isinstance(low[1].args[0], mir.Held)):
        return low[1].args[0]
    return None


def raised(body, found, contracts):
    if "FIDRQQ" not in omf.externals(found.records):
        return body
    local = module.defines(found.records, found.seg)
    read = {value for block in body.blocks for op in block.ops for value in op.uses}
    read.update(value for block in body.blocks for phi in block.phis for value in phi.incoming.values())
    blocks = []
    for block in body.blocks:
        definitions = {}
        ops = []
        for index, op in enumerate(block.ops):
            name = found.calls.get(op.at)
            width = _WIDTHS.get(name)
            contract = contracts.get(op.at)
            expected = runtime.contract(name) if width else None
            candidate = (op.kind is mir.Kind.CALL and width is not None
                         and name not in local and contract is not None and contract.established
                         and contract.writes is runtime.Memory.NONE
                         and contract.reads is runtime.Memory.NONE and contract.cleanup == 0
                         and contract.control is runtime.Control.RETURNS
                         and not (contract.enters_user_code or contract.raises_error or contract.error_handling)
                         and contract.inputs == expected.inputs
                         and contract.clobbers == expected.clobbers
                         and not any(value in read for value in op.defines))
            argument = _source(op, width, body.origin, definitions) if candidate else None
            source = definitions.get(argument.value) if isinstance(argument, mir.Held) else None
            load = None
            if source is not None:
                where, producer = source
                if (producer.kind is mir.Kind.LOAD and len(producer.args) == 1
                    and isinstance(producer.args[0], mir.Cell) and producer.args[0].ref.width == width
                    and all(not one.stores and not one.barrier and one.kind is not mir.Kind.CALL
                            for one in block.ops[where + 1:index])):
                    load = producer
                    argument = load.args[0]
            if argument is not None:
                ref = argument.ref if isinstance(argument, mir.Cell) else None
                uses = (argument.value,) if isinstance(argument, mir.Held) else tuple(
                    value for value in (ref.base, ref.segment) if value is not None)
                op = replace(op, kind=mir.Kind.FLOAD, op=ir.Operation.FLOAT_LOAD, name="fild",
                             args=(argument,), results=(mir.Opaque(ir.St(0), "st0"),),
                             uses=uses, defines=(), loads=(ref,) if ref is not None else (),
                             stores=(), merges={}, node=None, made=None, raised=None, stack=1,
                             symbol=ref is not None)
                if load is not None:
                    found.refs.update(mir._referenced(replace(body, blocks=(replace(block, ops=(load,)),)), found))
                    if load.id in found.refs:
                        found.refs[op.id] = found.refs[load.id]
                # The helper supplied its own emulator dispatch. Its replacement
                # must retain that protocol, independent of the call's opcode.
                found.float_protocols[op.id] = 0x34
            ops.append(op)
            definitions.update({value: (index, op) for value in op.defines})
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))
