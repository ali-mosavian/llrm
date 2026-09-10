"""Assign floating LIR values to the target register stack."""

from dataclasses import replace
from collections import Counter, defaultdict, deque

from qbopt.model import ir, lir
from qbopt.model.passes import LIRTransform


def allocated(body: lir.LirBody, frame=None) -> lir.LirBody:
    from qbopt.backend.lower import Unlowered

    floating = {arg.value for block in body.blocks for one in block.insns if one.what
                for arg in (*one.what.sources, *one.what.dests)
                if isinstance(arg, ir.Held) and arg.width == 10}
    if not floating:
        return body
    integer_readers = set(body.pins)
    for block in body.blocks:
        integer_readers.update(value for phi in block.phis for _, value in phi.incoming)
        for one in block.insns:
            integer_readers.update(one.uses)
            if one.what is not None:
                integer_readers.update(arg.value for arg in one.what.sources if isinstance(arg, ir.Held))
    unknown_readers = any(one.what is None or one.what.op is ir.Operation.BARRIER for one in body.insns)
    predecessors = {block.at: set() for block in body.blocks}
    for block in body.blocks:
        for successor in block.succ:
            if successor in predecessors:
                predecessors[successor].add(block.at)
    order = tuple(block.at for block in body.blocks)
    next_blocks = {block.at: block.succ[0] for block in body.blocks
                   if len(block.succ) == 1 and block.succ[0] != body.entry
                   and predecessors.get(block.succ[0]) == {block.at}}
    at_of = {block.at: block for block in body.blocks}
    destinations = set(next_blocks.values())
    roots = [at for at in order if at not in destinations]
    scheduled, seen = [], set()
    for root in (*roots, *order):
        at = root
        while at is not None and at not in seen:
            scheduled.append(at_of[at])
            seen.add(at)
            at = next_blocks.get(at)
    body = replace(body, blocks=tuple(scheduled))
    continues = {
        index for index, (block, following) in enumerate(zip(body.blocks, body.blocks[1:]))
        if next_blocks.get(block.at) == following.at
    }
    if any(phi.result in floating or any(value in floating for _, value in phi.incoming)
           for block in body.blocks for phi in block.phis):
        raise Unlowered("floating phi requires cross-block allocation")
    from qbopt.backend.floatregions import bridged
    regions, region = {}, 0
    for index, block in enumerate(body.blocks):
        if index - 1 not in continues:
            region += 1
        regions[block.at] = region
    body = bridged(body, regions, frame)
    floating = {arg.value for block in body.blocks for one in block.insns if one.what
                for arg in (*one.what.sources, *one.what.dests)
                if isinstance(arg, ir.Held) and arg.width == 10}
    blocks = []
    stack: list[int] = []
    spilled: dict[int, ir.Mem] = {}
    remaining = Counter()
    next_uses = defaultdict(deque)
    for index, block in enumerate(body.blocks):
        if index - 1 not in continues:
            end = index
            while end in continues:
                end += 1
            next_uses = defaultdict(deque)
            region = (one for member in body.blocks[index:end + 1] for one in member.insns)
            for position, instruction in enumerate(region):
                if instruction.what:
                    for arg in instruction.what.sources:
                        if isinstance(arg, ir.Held) and arg.width == 10:
                            next_uses[arg.value].append(position)
            remaining = Counter({value: len(uses) for value, uses in next_uses.items()})
        insns = []
        for one in block.insns:
            what = one.what
            converted_result = None
            if (what is not None and what.op is ir.Operation.FLOAT_STORE and what.name == "fistp"
                and len(what.sources) == len(what.dests) == 1
                and isinstance(what.dests[0], ir.Held) and what.dests[0].width in (2, 4)):
                if frame is None:
                    raise Unlowered("floating-to-integer conversion requires an owned frame")
                converted_result = what.dests[0]
                converted_cell = frame.cell(("integer-conversion", converted_result.value), converted_result.width)
                insns.append(lir.Insn(one.at, (one.at, one.at),
                    ir.Semantics(ir.Operation.NOTHING, "wait", (), ()), (), ()))
                one = replace(one, what=replace(what, dests=(converted_cell,)),
                    defines=tuple(value for value in one.defines if value != converted_result.value))
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
                if (stack or any(remaining[value] for value in spilled)) and (what is None or what.op in (ir.Operation.CALL, ir.Operation.BARRIER)
                              or any(isinstance(arg, ir.St) for arg in (*what.sources, *what.dests))):
                    raise Unlowered("floating stack crosses an unmodelled instruction")
                insns.append(one)
                continue

            if (what.op is ir.Operation.FLOAT_ARITH and what.name in ("fadd", "fmul", "fsub", "fdiv")
                and len(what.sources) == 2
                and all(isinstance(arg, ir.Held) and arg.width == 10 for arg in what.sources)
                and what.sources[0] != what.sources[1] and remaining[what.sources[1].value] == 1):
                reverse = not stack or stack[0] != what.sources[1].value
                names = {"fadd": ("faddp", "faddp"), "fmul": ("fmulp", "fmulp"),
                         "fsub": ("fsubp", "fsubrp"), "fdiv": ("fdivp", "fdivrp")}
                what = replace(what, op=ir.Operation.FLOAT_ARITH_POP, name=names[what.name][reverse],
                               sources=what.sources[::-1] if reverse else what.sources)
            used = Counter(arg.value for arg in what.sources if isinstance(arg, ir.Held) and arg.width == 10)
            for value, count in used.items():
                for _ in range(count):
                    next_uses[value].popleft()
            missing = [value for value in used if value not in stack]
            retained_store = (what.op is ir.Operation.FLOAT_STORE and what.name == "fstp"
                              and len(what.sources) == len(what.dests) == 1
                              and isinstance(what.sources[0], ir.Held)
                              and isinstance(what.dests[0], ir.Mem) and what.dests[0].width in (4, 8)
                              and remaining[what.sources[0].value] > used[what.sources[0].value])
            if retained_store:
                what = replace(what, name="fst")
            extra = 0
            match what.op:
                case ir.Operation.FLOAT_LOAD:
                    extra = 1
                case ir.Operation.FLOAT_STORE | ir.Operation.FLOAT_ARITH | ir.Operation.FLOAT_UNARY:
                    if what.sources and isinstance(what.sources[0], ir.Held):
                        value = what.sources[0].value
                        extra = int(not retained_store and remaining[value] > used[value])
                case ir.Operation.FLOAT_ARITH_POP:
                    operands = [arg.value for arg in what.sources if isinstance(arg, ir.Held) and arg.width == 10]
                    extra = sum(remaining[value] > used[value] for value in operands)
                    if len(operands) == 2 and operands[0] == operands[1]:
                        extra = max(extra, 1)
            while len(stack) + len(missing) + extra > 8:
                if frame is None:
                    raise Unlowered("floating spill requires an owned frame")
                victim = max((slot for slot in range(len(stack)) if stack[slot] not in used),
                             key=lambda slot: next_uses[stack[slot]][0] if next_uses[stack[slot]] else float("inf"),
                             default=None)
                if victim is None:
                    raise Unlowered("floating instruction requires too many stack operands")
                if victim:
                    operands = ir.St(0), ir.St(victim)
                    insns.append(lir.Insn(one.at, (one.at, one.at),
                        ir.Semantics(ir.Operation.EXCHANGE, "fxch", operands, operands), (), ()))
                    stack[0], stack[victim] = stack[victim], stack[0]
                value = stack.pop(0)
                cell = frame.cell(("floating", value), 10)
                spilled[value] = cell
                insns.append(lir.Insn(one.at, (one.at, one.at),
                    ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (ir.St(0),)), (), ()))
            for value in missing:
                if value not in spilled:
                    raise Unlowered("floating stack input is unavailable")
                insns.append(lir.Insn(one.at, (one.at, one.at),
                    ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (ir.St(0),), (spilled[value],)), (), ()))
                stack.insert(0, value)

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
            arithmetic_slot = 0
            if what.op in (ir.Operation.FLOAT_STORE, ir.Operation.FLOAT_ARITH, ir.Operation.FLOAT_UNARY):
                if not retained_store and inputs and inputs[0] == ir.St(0) and remaining[stack[0]]:
                    if len(stack) == 8:
                        raise Unlowered("floating stack requires a spill to preserve a live value")
                    operands = (ir.St(0),)
                    insns.append(lir.Insn(one.at, (one.at, one.at),
                        ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", operands, operands), (), ()))
                    stack.insert(0, stack[0])
                    inputs = tuple(map(source, what.sources))
                    if (what.op is ir.Operation.FLOAT_ARITH and what.name in ("fadd", "fmul")
                        and len(what.sources) == 2 and what.sources[0] == what.sources[1]
                        and len(what.dests) == 1 and isinstance(what.dests[0], ir.Held)
                        and next_uses[stack[0]] and next_uses[what.dests[0].value]
                        and next_uses[stack[0]][0] < next_uses[what.dests[0].value][0]):
                        inputs = ir.St(1), ir.St(0)
                        arithmetic_slot = 1
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
                    delta, slot = (0 if retained_store else -1), None
                    if inputs != (ir.St(0),):
                        raise Unlowered("floating stack store requires an exchange")
                case ir.Operation.FLOAT_ARITH | ir.Operation.FLOAT_UNARY:
                    delta, slot = 0, arithmetic_slot
                    if not inputs or inputs[0] != ir.St(slot):
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
            if converted_result is not None:
                insns.append(lir.Insn(one.at, (one.at, one.at),
                    ir.Semantics(ir.Operation.NOTHING, "wait", (), ()), (), ()))
                if unknown_readers or converted_result.value in integer_readers:
                    insns.append(lir.Insn(one.at, (one.at, one.at),
                        ir.Semantics(ir.Operation.MOVE, "mov", (converted_result,), (converted_cell,)),
                        (converted_result.value,), (), widths=((converted_result.value, converted_result.width),)))
        if stack and index not in continues:
            raise Unlowered("floating stack live-out requires cross-block allocation")
        if index not in continues:
            spilled.clear()
        blocks.append(replace(block, insns=tuple(insns)))
    allocated_blocks = {block.at: block for block in blocks}
    return replace(body, blocks=tuple(allocated_blocks[at] for at in order))


class FloatAlloc(LIRTransform):
    name = "floatalloc"

    def __init__(self, frame=None):
        self.frame = frame

    def transform(self, body):
        return allocated(body, self.frame)
