"""A bounded, exact path proof for numeric array accesses at the raise boundary.

No unknown branch is selected. Every access is checked before its effect is
interpreted, so memory disjointness is a conclusion, not a loop assumption.
Failure or exhaustion discards the entire proof. This recognizes finite constant
control flow; it is not a substitute for general symbolic range analysis.
"""

from dataclasses import dataclass, replace

from qbopt.analysis import consts
from qbopt.model import ir, mir
from qbopt.objectfile.module import Addr, Space


@dataclass(frozen=True)
class Pointer:
    offset: int


def proven(body: mir.MirBody, *, limit: int = 10000) -> mir.MirBody:
    allocations = [op for block in body.blocks for op in block.ops if op.array]
    if len(allocations) != 1 or allocations[0].array.replaces or not allocations[0].memory_values:
        return body
    allocation = allocations[0]
    request = allocation.array
    if any(low != 0 for low, _ in request.bounds):
        return body
    extent = request.element_width
    for low, high in request.bounds:
        extent *= high - low + 1
    if not 0 < extent < 32768:
        return body
    checked = _walk(body, allocation, extent, limit)
    if not checked:
        return body

    def reference(ref: mir.MemRef) -> mir.MemRef:
        return replace(ref, allocation=request.descriptor) if ref in checked else ref

    def argument(arg: mir.Arg) -> mir.Arg:
        return mir.Cell(reference(arg.ref)) if isinstance(arg, mir.Cell) else arg

    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(
                    replace(
                        op,
                        loads=tuple(map(reference, op.loads)),
                        stores=tuple(map(reference, op.stores)),
                        args=tuple(map(argument, op.args)),
                        results=tuple(map(argument, op.results)),
                    )
                    for op in block.ops
                ),
            )
            for block in body.blocks
        ),
    )


def _walk(body: mir.MirBody, allocation: mir.Op, extent: int, limit: int) -> set[mir.MemRef]:
    blocks = {block.at: block for block in body.blocks}
    values = {}
    memory = {}
    segments = {}
    comparisons = {}
    checked = set()
    active = False
    descriptor = allocation.array.descriptor
    base = descriptor.offset + descriptor.addend
    descriptor_end = base + 14 + 4 * len(allocation.array.bounds)
    previous, current = None, body.entry

    def no_more_elements(at):
        pending, seen = [at], set()
        while pending:
            at = pending.pop()
            if at in seen:
                continue
            seen.add(at)
            if at not in blocks:
                return False
            block = blocks[at]
            if any(ref.addr and ref.addr.space is Space.FAR for op in block.ops for ref in op.loads + op.stores):
                return False
            pending.extend(block.succ)
        return True

    def address(ref: mir.MemRef):
        resolved = mir._symbolic_ref(ref)
        if resolved.addr is None:
            raise ValueError
        if resolved.base is None and resolved.segment is None and resolved.addr.space is Space.SEGMENT:
            return resolved.addr
        if ref.addr.space is Space.FAR and active and segments.get(ref.addr.segment) is True:
            pointer = values.get(ref.base)
            if isinstance(pointer, Pointer) and 0 <= pointer.offset + ref.addr.disp <= extent - ref.width:
                checked.add(ref)
                return ("element", pointer.offset + ref.addr.disp)
        raise ValueError

    def read(arg):
        match arg:
            case mir.Const(n=number, width=width):
                return consts.masked(number, width)
            case mir.Held(value=value, width=width):
                if width != 2:
                    raise ValueError
                result = values.get(value)
                return consts.masked(result, width) if isinstance(result, int) else result
            case mir.Cell(ref=ref):
                if ref.width != 2:
                    raise ValueError
                return memory.get((address(ref), ref.width))
        return None

    try:
        while current in blocks:
            block = blocks[current]
            incoming = {phi.result: values.get(phi.incoming.get(previous)) for phi in block.phis}
            values.update(incoming)
            following = block.succ[0] if len(block.succ) == 1 else None
            for op in block.ops:
                limit -= 1
                if limit < 0:
                    raise ValueError
                if op.kind is mir.Kind.CALL:
                    if op is allocation and not active:
                        active = True
                        memory = {(ref.addr, ref.width): value.n for ref, value in op.memory_values}
                        memory[(Addr(Space.SEGMENT, base + 10, descriptor.index), 2)] = Pointer(0)
                        memory[(Addr(Space.SEGMENT, base + 2, descriptor.index), 2)] = True
                        values.clear()
                        segments.clear()
                        continue
                    # A terminal block may print results, but cannot revisit an access.
                    if no_more_elements(block.at):
                        return checked
                    raise ValueError
                if op.kind in (mir.Kind.ARG, mir.Kind.JUMP, mir.Kind.NOTHING):
                    continue
                if op.barrier or op.kind not in {
                    *consts.ARITH,
                    mir.Kind.COPY,
                    mir.Kind.LOAD,
                    mir.Kind.STORE,
                    mir.Kind.INCREMENT,
                    mir.Kind.BRANCH,
                }:
                    raise ValueError
                if op.kind is mir.Kind.BRANCH:
                    flags = [value for value in op.uses if value.flags]
                    if len(flags) != 1 or flags[0] not in comparisons:
                        raise ValueError
                    left, right, width = comparisons[flags[0]]
                    sign = 1 << (width * 8 - 1)
                    left, right = (left ^ sign) - sign, (right ^ sign) - sign
                    match op.test:
                        case mir.Kind.LE:
                            taken = left <= right
                        case mir.Kind.LT:
                            taken = left < right
                        case mir.Kind.GE:
                            taken = left >= right
                        case mir.Kind.GT:
                            taken = left > right
                        case mir.Kind.EQ:
                            taken = left == right
                        case mir.Kind.NE:
                            taken = left != right
                        case _:
                            raise ValueError
                    others = [at for at in block.succ if at != op.target]
                    if len(others) != 1:
                        raise ValueError
                    following = op.target if taken else others[0]
                    continue
                args = [read(arg) for arg in op.args]
                for ref in op.loads:
                    address(ref)
                result = None
                if op.kind in (mir.Kind.COPY, mir.Kind.LOAD, mir.Kind.STORE) and len(args) == 1:
                    result = args[0]
                elif (
                    op.kind is mir.Kind.ADD
                    and len(args) == 2
                    and isinstance(args[0], int)
                    and isinstance(args[1], Pointer)
                ):
                    result = Pointer(args[0] + args[1].offset)
                elif len(args) == 2 and all(type(arg) is int for arg in args) and op.kind in consts.ARITH:
                    result = consts.ARITH[op.kind](*args)
                elif op.kind is mir.Kind.INCREMENT and len(args) == 1 and type(args[0]) is int:
                    result = args[0] + 1
                for value in op.defines:
                    values.pop(value, None)
                    comparisons.pop(value, None)
                if op.op is ir.Operation.COMPARE and len(args) == 2 and all(type(arg) is int for arg in args):
                    width = op.args[0].width
                    if width != 2 or op.args[1].width != width:
                        raise ValueError
                    for value in op.defines:
                        if value.flags:
                            comparisons[value] = (*args, width)
                for index, output in enumerate(op.results):
                    if isinstance(output, mir.Held):
                        if output.width != 2:
                            raise ValueError
                        values[output.value] = consts.masked(result, output.width) if type(result) is int else result
                        if index and op.kind is mir.Kind.MUL:
                            values[output.value] = None
                    elif isinstance(output, mir.Opaque) and isinstance(output.what, ir.Reg):
                        segments[output.what.register] = result
                for ref in op.stores:
                    target = address(ref)
                    if ref.width != 2 or isinstance(target, Addr) and target.index != descriptor.index:
                        raise ValueError
                    if (
                        isinstance(target, Addr)
                        and target.index == descriptor.index
                        and target.disp < descriptor_end
                        and base < target.disp + ref.width
                    ):
                        raise ValueError
                    for key in list(memory):
                        location, width = key
                        if isinstance(target, Addr) and isinstance(location, Addr):
                            overlaps = (
                                location.index == target.index
                                and location.disp < target.disp + ref.width
                                and target.disp < location.disp + width
                            )
                        elif isinstance(target, tuple) and isinstance(location, tuple):
                            overlaps = location[1] < target[1] + ref.width and target[1] < location[1] + width
                        else:
                            overlaps = False
                        if overlaps:
                            del memory[key]
                    memory[target, ref.width] = consts.masked(result, ref.width) if type(result) is int else result
            if not block.succ:
                return checked
            if following is None:
                raise ValueError
            previous, current = current, following
    except (ValueError, AttributeError):
        return set()
    return set()
