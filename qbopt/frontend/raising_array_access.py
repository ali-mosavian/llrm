"""Expose numeric single-segment array addressing as scalar MIR arithmetic.

Static allocations and established non-huge far allocations are recognized.
Huge/string layouts remain unsupported, not guessed pointers.
"""

from dataclasses import dataclass, replace
from math import prod

from iced_x86 import Register

from qbopt.analysis import consts, ssa
from qbopt.frontend.raising_calls import _capture, _discarded
from qbopt.model import ir, mir
from qbopt.objectfile import module, omf
from qbopt.objectfile.module import Addr, Space


@dataclass(frozen=True)
class Descriptor:
    data: mir.Symbol | mir.MemRef
    selector: mir.MemRef
    width: int
    dimensions: tuple[tuple[int | mir.MemRef, int | mir.MemRef], ...]


def descriptor(found, symbol):
    if not isinstance(symbol, mir.Symbol) or symbol.space is not Space.SEGMENT or symbol.width != 2:
        return None
    start = symbol.offset + symbol.addend
    segments = omf.segments(found.records)
    if not 0 < symbol.index < len(segments) or segments[symbol.index] is None:
        return None
    size = segments[symbol.index][1]
    if not 0 <= start <= size - 18:
        return None
    data = omf.segment_image(found.records, symbol.index, size)
    rank, features = data[start + 8:start + 10]
    if not 1 <= rank <= 8 or features != 0x40 or start + 14 + 4 * rank > size:
        return None
    fixups = [one for one in omf.fixups(found.records) if one.seg == symbol.index]
    pointer = [one for one in fixups if one.offset == start and one.loc == omf.LOC_PTR32
               and one.target == "segment"]
    if len(pointer) != 1 or any(start + 8 <= one.offset < start + 14 + 4 * rank
                                and one.offset != start + 10 for one in fixups):
        return None
    pointer = pointer[0]
    if pointer.index not in found.dgroup or not 0 < pointer.index < len(segments):
        return None
    width = int.from_bytes(data[start + 12:start + 14], "little")
    dimensions = tuple((int.from_bytes(data[at:at + 2], "little"),
                        int.from_bytes(data[at + 2:at + 4], "little", signed=True))
                       for at in range(start + 14, start + 14 + 4 * rank, 4))
    if width not in (1, 2, 4, 8) or any(count == 0 for count, _ in dimensions):
        return None
    offset = pointer.disp + int.from_bytes(data[start:start + 2], "little")
    extent = width * prod(count for count, _ in dimensions)
    target = segments[pointer.index]
    if target is None or not 0 <= offset <= min(65536, target[1]) - extent:
        return None
    return Descriptor(mir.Symbol(Space.SEGMENT, pointer.index, offset, 2),
                      mir.MemRef(Addr(Space.SEGMENT, start + 2, symbol.index), 2), width, dimensions)


def dynamic(body, symbol):
    if not isinstance(symbol, mir.Symbol):
        return None
    allocations = [op for block in body.blocks for op in block.ops
                   if op.array and op.array.descriptor == symbol]
    if len(allocations) != 1 or allocations[0].array.replaces:
        return None
    allocation = allocations[0]
    start = symbol.offset + symbol.addend

    def field(offset, width=2):
        return mir.MemRef(Addr(symbol.space, start + offset, symbol.index), width)

    # All three DDIM implementations zero the base offset for numeric FAR
    # arrays and reject sizes above 64K. Unlike HUGE, a valid access never
    # needs selector carry. Load mutable fields at each access; do not turn
    # allocation-time bounds or a movable heap address into eternal constants.
    if dict(allocation.memory_values).get(field(9, 1)) != mir.Const(1, 1):
        return None
    request = allocation.array
    return Descriptor(field(0), field(2), request.element_width,
                      tuple((field(14 + 4 * index), field(16 + 4 * index))
                            for index in range(len(request.bounds))))


def native(body, found, *, bounds_checks=False):
    if bounds_checks:
        return body
    local = module.defines(found.records, found.seg)
    calls = {at: name for at, name in found.calls.items() if name not in local}
    known = consts.known(body)
    definitions = {value: op for block in body.blocks for op in block.ops for value in op.defines}
    read = {value for block in body.blocks for op in block.ops for value in op.uses}
    read.update(value for block in body.blocks for phi in block.phis for value in phi.incoming.values())
    values = set(ssa.values(body)) | set(body.origin)
    serial = max((value.id for value in values), default=0)
    variable = max((value.variable for value in values), default=0)

    def fresh(at):
        nonlocal serial, variable
        serial += 1
        variable += 1
        return mir.Held(mir.Value(serial, at, variable=variable, version=1), 2)

    blocks = []
    for block in body.blocks:
        arguments, ops = [], []
        for op in block.ops:
            if (op.kind is mir.Kind.ARG and len(op.args) == 1
                and (op.args[0].ref.width if isinstance(op.args[0], mir.Cell)
                     else getattr(op.args[0], "width", 0)) == 2):
                arguments.append(len(ops))
            elif op.kind is mir.Kind.CALL and calls.get(op.at) == "B$HARY":
                source = definitions.get(op.args[0].value) if len(op.args) == 1 and isinstance(op.args[0], mir.Held) else None
                symbol = source.args[0] if source and source.kind is mir.Kind.COPY and len(source.args) == 1 else None
                shape = descriptor(found, symbol) or dynamic(body, symbol)
                outputs = [value for value in op.defines if not value.flags]
                rank = ops[arguments[-1]].args[0] if arguments else None
                if isinstance(rank, mir.Held) and (fact := known.get(rank.value)) and fact.width >= 2:
                    rank = mir.Const(fact.n & 0xffff, 2)
                # QB can reuse a register for rank across debug calls. In
                # unchecked mode the static descriptor and complete argument
                # group establish arity; runtime rank validation is omitted.
                valid = (shape is not None and (rank == mir.Const(len(shape.dimensions), 2)
                         or isinstance(rank, mir.Held) and rank.width == 2)
                         and len(arguments) == len(shape.dimensions) + 1
                         and len(outputs) == 1 and body.origin.get(outputs[0]) == Register.EBX
                         and not any(value.flags and value in read for value in op.defines))
                if valid:
                    indices = []
                    for index in arguments[:-1]:
                        push = ops[index]
                        held = fresh(push.at)
                        ops[index] = _capture(push, push.args[0], held)
                        indices.append(held)
                    ops[arguments[-1]] = _discarded(ops[arguments[-1]])
                    expanded = []

                    def loaded(source):
                        if not isinstance(source, mir.MemRef):
                            return mir.Const(source, 2) if isinstance(source, int) else source
                        result = fresh(op.at)
                        expanded.append(mir.Op(op.at, ir.Operation.MOVE, "mov", (result.value,), (),
                            kind=mir.Kind.LOAD, args=(mir.Cell(source),), results=(result,),
                            loads=(source,), covers=(op.at, op.at), symbol=True))
                        return result

                    def arithmetic(kind, left, right, result=None):
                        result = result or fresh(op.at)
                        uses = tuple(arg.value for arg in (left, right) if isinstance(arg, mir.Held))
                        expanded.append(mir.Op(op.at, ir.Operation.BINARY, kind.value,
                            (result.value,), uses, kind=kind, args=(left, right), results=(result,),
                            covers=(op.at, op.at), symbol=isinstance(right, mir.Symbol)))
                        return result

                    offset = None
                    for index, (count, lower) in zip(reversed(indices), shape.dimensions):
                        adjusted = arithmetic(mir.Kind.SUB, index, loaded(lower))
                        offset = adjusted if offset is None else arithmetic(mir.Kind.ADD,
                            arithmetic(mir.Kind.MUL, offset, loaded(count)), adjusted)
                    offset = arithmetic(mir.Kind.MUL, offset, mir.Const(shape.width, 2))
                    arithmetic(mir.Kind.ADD, offset, loaded(shape.data), mir.Held(outputs[0], 2))
                    expanded.append(mir.Op(op.at, ir.Operation.MOVE, "mov", (), (),
                        kind=mir.Kind.LOAD, args=(mir.Cell(shape.selector),),
                        results=(mir.Opaque(ir.Reg(Register.ES, 2), "es"),), loads=(shape.selector,),
                        covers=(op.at, op.at), symbol=True))
                    expanded[0] = replace(expanded[0], covers=op.covers, extra_covers=op.extra_covers)
                    ops.extend(expanded)
                    arguments.clear()
                    continue
                arguments.clear()
            elif op.kind not in (mir.Kind.COPY, mir.Kind.NOTHING):
                arguments.clear()
            ops.append(op)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))
