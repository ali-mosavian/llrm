"""Assign floating LIR values to the target register stack."""

from dataclasses import replace
from collections import Counter

from qbopt.model import ir, lir
from qbopt.model.passes import LIRTransform


def allocated(body: lir.LirBody, frame=None) -> lir.LirBody:
    from qbopt.backend.lower import Unlowered

    floating = {arg.value for block in body.blocks for one in block.insns if one.what
                for arg in (*one.what.sources, *one.what.dests)
                if isinstance(arg, ir.Held) and arg.width == 10}
    if not floating:
        return body
    blocks = []
    for block in body.blocks:
        if any(phi.result in floating or any(value in floating for _, value in phi.incoming)
               for phi in block.phis):
            raise Unlowered("floating phi requires cross-block allocation")
        stack: list[int] = []
        remaining = Counter(arg.value for one in block.insns if one.what for arg in one.what.sources
                            if isinstance(arg, ir.Held) and arg.width == 10)
        insns = []
        for one in block.insns:
            what = one.what
            if (what is not None and what.op is ir.Operation.FLOAT_LOAD and what.name == "fild"
                and len(what.sources) == len(what.dests) == 1
                and isinstance(what.sources[0], (ir.Held, ir.Imm))
                and what.sources[0].width in (2, 4) and isinstance(what.dests[0], ir.Held)):
                if frame is None:
                    raise Unlowered("integer-to-floating conversion requires an owned frame")
                value = what.sources[0]
                cell = frame.cell(what.dests[0].value, value.width)
                uses = (value.value,) if isinstance(value, ir.Held) else ()
                insns.append(lir.Insn(one.at, (one.at, one.at),
                    ir.Semantics(ir.Operation.MOVE, "mov", (cell,), (value,)), (), uses))
                one = replace(one, what=replace(what, sources=(cell,)),
                              uses=tuple(arg for arg in one.uses if arg not in uses))
                what = one.what
            if what is None or not any(isinstance(arg, ir.Held) and arg.width == 10
                                      for arg in (*what.sources, *what.dests)):
                if floating.intersection((*one.uses, *one.defines)):
                    raise Unlowered("floating value used by an unmodelled instruction")
                if stack and (what is None or what.op in (ir.Operation.CALL, ir.Operation.BARRIER)
                              or any(isinstance(arg, ir.St) for arg in (*what.sources, *what.dests))):
                    raise Unlowered("floating stack crosses an unmodelled instruction")
                insns.append(one)
                continue

            def source(arg):
                if not isinstance(arg, ir.Held) or arg.width != 10:
                    return arg
                if arg.value not in stack:
                    raise Unlowered("floating stack input is unavailable")
                return ir.St(stack.index(arg.value))

            inputs = tuple(map(source, what.sources))
            remaining.subtract(arg.value for arg in what.sources if isinstance(arg, ir.Held) and arg.width == 10)
            match what.op:
                case ir.Operation.FLOAT_STORE | ir.Operation.FLOAT_ARITH | ir.Operation.FLOAT_UNARY:
                    required = inputs[0] if inputs else None
                case ir.Operation.FLOAT_ARITH_POP:
                    required = inputs[1] if len(inputs) == 2 else None
                case _:
                    required = None
            if isinstance(required, ir.St) and required.index:
                operands = (ir.St(0), required)
                insns.append(lir.Insn(one.at, (one.at, one.at),
                    ir.Semantics(ir.Operation.EXCHANGE, "fxch", operands, operands), (), ()))
                stack[0], stack[required.index] = stack[required.index], stack[0]
                inputs = tuple(map(source, what.sources))
            if what.op in (ir.Operation.FLOAT_STORE, ir.Operation.FLOAT_ARITH, ir.Operation.FLOAT_UNARY):
                if inputs and inputs[0] == ir.St(0) and remaining[stack[0]]:
                    if len(stack) == 8:
                        raise Unlowered("floating stack requires a spill to preserve a live value")
                    operands = (ir.St(0),)
                    insns.append(lir.Insn(one.at, (one.at, one.at),
                        ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", operands, operands), (), ()))
                    stack.insert(0, stack[0])
                    inputs = tuple(map(source, what.sources))
            elif what.op is ir.Operation.FLOAT_ARITH_POP:
                if len(inputs) != 2 or not all(isinstance(arg, ir.St) for arg in inputs):
                    raise Unlowered("floating popping arithmetic requires two stack operands")
                left, right = (arg.index for arg in inputs)

                def duplicate(index):
                    if len(stack) == 8:
                        raise Unlowered("floating stack requires a spill to preserve a live value")
                    insns.append(lir.Insn(one.at, (one.at, one.at),
                        ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (ir.St(0),), (ir.St(index),)), (), ()))
                    stack.insert(0, stack[index])

                if remaining[stack[left]]:
                    duplicate(left)
                    left, right = 0, right + 1
                if remaining[stack[right]] or left == right:
                    duplicate(right)
                    left, right = left + 1, 0
                if right:
                    operands = (ir.St(0), ir.St(right))
                    insns.append(lir.Insn(one.at, (one.at, one.at),
                        ir.Semantics(ir.Operation.EXCHANGE, "fxch", operands, operands), (), ()))
                    stack[0], stack[right] = stack[right], stack[0]
                    if left == 0:
                        left = right
                inputs = ir.St(left), ir.St(0)
            match what.op:
                case ir.Operation.FLOAT_LOAD:
                    delta, slot = 1, 0
                case ir.Operation.FLOAT_STORE:
                    delta, slot = -1, None
                    if inputs != (ir.St(0),):
                        raise Unlowered("floating stack store requires an exchange")
                case ir.Operation.FLOAT_ARITH | ir.Operation.FLOAT_UNARY:
                    delta, slot = 0, 0
                    if not inputs or inputs[0] != ir.St(0):
                        raise Unlowered("floating stack arithmetic requires an exchange")
                case ir.Operation.FLOAT_ARITH_POP:
                    delta = -1
                    if len(inputs) != 2 or not isinstance(inputs[0], ir.St) or inputs[1] != ir.St(0):
                        raise Unlowered("floating stack popping arithmetic requires an exchange")
                    slot = inputs[0].index
                case _:
                    raise Unlowered("floating instruction has no allocation rule")
            outputs = []
            for arg in what.dests:
                if not isinstance(arg, ir.Held) or arg.width != 10:
                    outputs.append(arg)
                    continue
                if slot is None or arg.value in stack:
                    raise Unlowered("floating stack result is not a fresh value")
                outputs.append(ir.St(slot))
                if delta == 1:
                    if len(stack) == 8:
                        raise Unlowered("floating stack requires a spill")
                    stack.insert(0, arg.value)
                else:
                    if slot >= len(stack):
                        raise Unlowered("floating stack result has no slot")
                    stack[slot] = arg.value
            if delta == -1:
                if not stack:
                    raise Unlowered("floating stack pop has no value")
                stack.pop(0)
            insns.append(replace(one, what=replace(what, sources=inputs, dests=tuple(outputs)),
                uses=tuple(value for value in one.uses if value not in floating),
                defines=tuple(value for value in one.defines if value not in floating),
                widths=tuple((value, width) for value, width in one.widths if value not in floating)))
        if stack:
            raise Unlowered("floating stack live-out requires cross-block allocation")
        blocks.append(replace(block, insns=tuple(insns)))
    return replace(body, blocks=tuple(blocks))


class FloatAlloc(LIRTransform):
    name = "floatalloc"

    def __init__(self, frame=None):
        self.frame = frame

    def transform(self, body):
        return allocated(body, self.frame)
