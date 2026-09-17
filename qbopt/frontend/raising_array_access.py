"""Expose numeric array addressing as scalar MIR arithmetic.

Static allocations and established non-huge far allocations are recognized.
Huge numeric accesses use whole pointers; string layouts remain unsupported.
"""

from dataclasses import dataclass, replace
from math import prod

from iced_x86 import Register

from qbopt.analysis import consts, ssa
from qbopt.abi import runtime
from qbopt.frontend.raising_calls import _capture, _discarded
from qbopt.model import ir, mir
from qbopt.objectfile import module, omf
from qbopt.objectfile.module import Addr, Space

# qb/ir/prsid.asm: MAXDIM, shared with BASCOM; validated with all three BCs.
MAX_DIMENSIONS = 60


@dataclass(frozen=True)
class Descriptor:
    data: mir.Symbol | mir.MemRef
    selector: mir.MemRef
    width: int
    dimensions: tuple[tuple[int | mir.MemRef, int | mir.MemRef], ...]
    huge: bool = False


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
    if not 1 <= rank <= MAX_DIMENSIONS or features != 0x40 or start + 14 + 4 * rank > size:
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
    start = symbol.offset + symbol.addend

    def field(offset, width=2):
        return mir.MemRef(Addr(symbol.space, start + offset, symbol.index), width)

    allocations = [op for block in body.blocks for op in block.ops
                   if op.kind is mir.Kind.CALL and field(9, 1) in dict(op.memory_values)]
    if len(allocations) != 1:
        return None
    facts = dict(allocations[0].memory_values)

    # All three DDIM implementations zero the base offset for numeric FAR
    # arrays and reject sizes above 64K. Unlike HUGE, a valid access never
    # needs selector carry. Load mutable fields at each access; do not turn
    # allocation-time bounds or a movable heap address into eternal constants.
    rank, width = facts.get(field(8, 1)), facts.get(field(12))
    features = facts.get(field(9, 1))
    if (features not in (mir.Const(1, 1), mir.Const(2, 1), mir.Const(3, 1))
        or not isinstance(rank, mir.Const) or not isinstance(width, mir.Const)
        or not 1 <= rank.n <= MAX_DIMENSIONS or width.n not in (1, 2, 4, 8)):
        return None
    huge = bool(features.n & 2)
    return Descriptor(field(0, 4 if huge else 2), field(2), width.n,
                      tuple((field(14 + 4 * index), field(16 + 4 * index))
                            for index in range(rank.n)), huge)


def _overwrites_offset(body, op, value):
    if (op.kind is not mir.Kind.COPY or len(op.args) != 1 or len(op.results) != 1
        or not isinstance(op.args[0], (mir.Symbol, mir.Const, mir.Held)) or op.args[0].width != 2
        or isinstance(op.args[0], mir.Held) and op.args[0].value == value
        or not isinstance(op.results[0], mir.Held) or op.results[0].width != 2
        or op.merges != {value: op.results[0].value}):
        return False
    pending, visited = [op.results[0].value], set()
    while pending:
        result = pending.pop()
        if result in visited:
            continue
        visited.add(result)
        for block in body.blocks:
            if any(result in phi.incoming.values() for phi in block.phis):
                return False
            for later in block.ops:
                if (any(isinstance(arg, mir.Held) and arg.value == result and arg.width > 2 for arg in later.args)
                    or any(ref.base == result and ref.base_width > 2 for ref in (*later.loads, *later.stores))):
                    return False
                if result in later.merges:
                    pending.append(later.merges[result])
    return True


def _selector_dead(body, block, position, contracts):
    blocks = {one.at: one for one in body.blocks}
    pending, visited = [(block.at, position)], set()
    while pending:
        at, start = pending.pop()
        if (at, start) in visited:
            continue
        visited.add((at, start))
        current = blocks.get(at)
        if current is None:
            return False
        for later in current.ops[start:]:
            if later.kind is mir.Kind.NOTHING:
                continue
            if later.kind is mir.Kind.CALL:
                contract = contracts.get(later.at)
                if contract is None or contract.inputs is None or runtime.Reg.ES in contract.inputs:
                    return False
                if runtime.Reg.ES in contract.clobbers:
                    break
            else:
                effects = getattr(getattr(later, "node", None), "effects", None)
                if effects is None or effects.uses is None or Register.ES in effects.uses:
                    return False
                if effects.defs is not None and Register.ES in effects.defs:
                    break
        else:
            if not current.succ:
                return False
            pending.extend((successor, 0) for successor in current.succ)
    return True


def _whole_consumer(body, block, position, value, contracts):
    """Prove the helper's machine outputs are only one local memory address."""
    consumer_position = position + 1
    for consumer_position in range(position + 1, len(block.ops)):
        candidate = block.ops[consumer_position]
        if value in candidate.uses:
            break
        effects = getattr(getattr(candidate, "node", None), "effects", None)
        if (candidate.kind is mir.Kind.CALL or effects is None or effects.uses is None
            or effects.defs is None or Register.ES in effects.uses | effects.defs):
            return None
    else:
        return None
    consumer = block.ops[consumer_position]
    if consumer.kind not in (mir.Kind.LOAD, mir.Kind.STORE, mir.Kind.ARG):
        return None
    refs = consumer.loads if consumer.kind is not mir.Kind.STORE else consumer.stores
    if len(refs) != 1:
        return None
    ref = refs[0]
    if (ref.base != value or ref.width not in (1, 2, 4) or ref.addr is None
        or ref.addr.space is not Space.FAR or ref.addr.segment != Register.ES
        or ref.addr.disp != 0 or ref.segment is not None
        or any(isinstance(arg, mir.Held) and arg.value == value for arg in consumer.args)):
        return None
    if any(value in op.uses and op is not consumer
           and not (other is block and _overwrites_offset(body, op, value))
           for other in body.blocks for op in other.ops):
        return None
    if any(value in phi.incoming.values() for other in body.blocks for phi in other.phis):
        return None
    return (consumer_position, consumer) if _selector_dead(body, block, consumer_position + 1, contracts) else None


def _checked(shape, symbol, indices, memory):
    """Every subscript fits the descriptor currently in memory, not just its total extent."""
    if shape is None or not isinstance(shape.data, mir.MemRef) or not isinstance(symbol, mir.Symbol):
        return False

    def field(offset, width):
        return consts._cell(memory, mir.MemRef(Addr(symbol.space, symbol.offset + symbol.addend + offset,
                                                 symbol.index), width))

    if (field(8, 1) != consts.Known(len(shape.dimensions), 1)
        or field(12, 2) != consts.Known(shape.width, 2)
        or field(9, 1) not in tuple(consts.Known(feature, 1) for feature in ((2, 3) if shape.huge else (1,)))):
        return False
    if len(indices) != len(shape.dimensions):
        return False
    for index, (count, lower) in zip(reversed(indices), shape.dimensions):
        count = consts._cell(memory, count)
        lower = consts._cell(memory, lower)
        if any(fact is None or fact.width < 2 for fact in (index, count, lower)):
            return False
        signed = lambda number: ((number & 0xffff) ^ 0x8000) - 0x8000
        if not 0 <= signed(index.n) - signed(lower.n) < (count.n & 0xffff):
            return False
    return True


def native(body, found, *, bounds_checks=False):
    """Expose accesses until newly proven allocation identities unlock no further checks."""
    def remaining(body):
        return sum(op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$HARY"
                   for block in body.blocks for op in block.ops)

    pending = remaining(body)
    while pending:
        body = _native(body, found, bounds_checks=bounds_checks)
        following = remaining(body)
        if not bounds_checks or following >= pending:
            break
        pending = following
    return body


def _native(body, found, *, bounds_checks):
    local = module.defines(found.records, found.seg)
    calls = {at: name for at, name in found.calls.items() if name not in local}
    if not any(op.kind is mir.Kind.CALL and calls.get(op.at) == "B$HARY"
               for block in body.blocks for op in block.ops):
        return body

    def overwritten(op):
        if len(op.merges) == 1:
            value = next(iter(op.merges))
            if _overwrites_offset(body, op, value):
                return replace(op, merges={}, uses=tuple(one for one in op.uses if one != value))
        return op
    body = replace(body, blocks=tuple(replace(block, ops=tuple(map(overwritten, block.ops))) for block in body.blocks))
    body = ssa.pruned_phis(body, {value for block in body.blocks for op in block.ops for value in op.uses})
    contracts = runtime.for_module(found)
    known = consts.known(body)
    memory = consts.cells(body, found.dgroup, calls, known) if bounds_checks else {}
    argument_facts = {id(op): consts._operand(op, op.args[0], known, memory.get((block.at, index), {}))
                      for block in body.blocks for index, op in enumerate(block.ops)
                      if bounds_checks and op.kind is mir.Kind.ARG and len(op.args) == 1}
    definitions = {value: op for block in body.blocks for op in block.ops for value in op.defines}
    read = {value for block in body.blocks for op in block.ops for value in op.uses}
    read.update(value for block in body.blocks for phi in block.phis for value in phi.incoming.values())
    values = set(ssa.values(body)) | set(body.origin)
    serial = max((value.id for value in values), default=0)
    variable = max((value.variable for value in values), default=0)

    def fresh(at, width=2):
        nonlocal serial, variable
        serial += 1
        variable += 1
        return mir.Held(mir.Value(serial, at, variable=variable, version=1), width)

    blocks = []
    for block in body.blocks:
        arguments, ops = [], []
        replacements = {}
        for position, original in enumerate(block.ops):
            op = replacements.get(position, original)
            if isinstance(op, tuple):
                load, op = op
                ops.append(load)
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
                if bounds_checks:
                    valid = (valid and rank == mir.Const(len(shape.dimensions), 2)
                             and _checked(shape, symbol, [argument_facts.get(id(ops[index])) for index in arguments[:-1]],
                                          memory.get((block.at, position), {})))
                consumer = _whole_consumer(body, block, position, outputs[0], contracts) if valid and shape.huge else None
                valid = valid and (not shape.huge or consumer is not None)
                if consumer is not None:
                    consumer_position, consumer = consumer
                if valid:
                    indices = []
                    for index in arguments[:-1]:
                        push = ops[index]
                        held = fresh(push.at)
                        ops[index] = _capture(push, push.args[0], held)
                        indices.append(held)
                    ops[arguments[-1]] = _discarded(ops[arguments[-1]])
                    expanded = []
                    width = 4 if shape.huge else 2

                    def loaded(source):
                        if not isinstance(source, mir.MemRef):
                            return mir.Const(source, 2) if isinstance(source, int) else source
                        result = fresh(op.at, source.width)
                        expanded.append(mir.Op(op.at, ir.Operation.MOVE, "mov", (result.value,), (),
                            kind=mir.Kind.LOAD, args=(mir.Cell(source),), results=(result,),
                            loads=(source,), symbol=True))
                        return result

                    def arithmetic(kind, left, right, result=None):
                        result = result or fresh(op.at, width)
                        uses = tuple(arg.value for arg in (left, right) if isinstance(arg, mir.Held))
                        expanded.append(mir.Op(op.at, ir.Operation.BINARY, kind.value,
                            (result.value,), uses, kind=kind, args=(left, right), results=(result,),
                            symbol=isinstance(right, mir.Symbol)))
                        return result

                    def extended(source, unsigned=False):
                        source = loaded(source)
                        if width == 2:
                            return source
                        if isinstance(source, mir.Const):
                            return mir.Const(source.n & 0xffff if unsigned else source.n, 4)
                        result = fresh(op.at, 4)
                        expanded.append(mir.Op(op.at, ir.Operation.EXTEND, "sign_extend",
                            (result.value,), (source.value,), kind=mir.Kind.SIGN_EXTEND,
                            args=(source,), results=(result,)))
                        return arithmetic(mir.Kind.AND, result, mir.Const(0xffff, 4)) if unsigned else result

                    offset = None
                    for index, (count, lower) in zip(reversed(indices), shape.dimensions):
                        adjusted = arithmetic(mir.Kind.SUB, extended(index), extended(lower))
                        offset = adjusted if offset is None else arithmetic(mir.Kind.ADD,
                            arithmetic(mir.Kind.MUL, offset, extended(count, unsigned=True)), adjusted)
                    offset = arithmetic(mir.Kind.MUL, offset, mir.Const(shape.width, width))
                    if shape.huge:
                        pointer = arithmetic(mir.Kind.PTR_OFFSET, loaded(shape.data), offset)
                        for following, later in enumerate(block.ops[position + 2:], position + 2):
                            if outputs[0] in later.uses and _overwrites_offset(body, later, outputs[0]):
                                replacements[following] = replace(later, merges={},
                                    uses=tuple(value for value in later.uses if value != outputs[0]))
                        ref = mir.MemRef(None, (consumer.loads or consumer.stores)[0].width,
                                         base=pointer.value, pointer=True)
                        def cell(arg):
                            return mir.Cell(ref) if isinstance(arg, mir.Cell) else arg
                        changed = mir.detached(consumer, name="mov", op=ir.Operation.MOVE,
                            args=tuple(map(cell, consumer.args)), results=tuple(map(cell, consumer.results)),
                            uses=tuple(pointer.value if value == outputs[0] else value for value in consumer.uses),
                            loads=(ref,) if consumer.loads else (), stores=(ref,) if consumer.kind is mir.Kind.STORE else (),
                            merges={}, symbol=True)
                        if consumer.kind is mir.Kind.ARG:
                            value = fresh(consumer.at, ref.width)
                            load = mir.source_free(changed, kind=mir.Kind.LOAD, defines=(value.value,),
                                results=(value,), stores=(), id=None, raised=None)
                            argument = replace(consumer, args=(value,), uses=(value.value,), loads=())
                            replacements[consumer_position] = (load, argument)
                        else:
                            replacements[consumer_position] = changed
                    else:
                        arithmetic(mir.Kind.ADD, offset, loaded(shape.data), mir.Held(outputs[0], 2))
                        expanded.append(mir.Op(op.at, ir.Operation.MOVE, "mov", (), (),
                            kind=mir.Kind.LOAD, args=(mir.Cell(shape.selector),),
                            results=(mir.Opaque(ir.Reg(Register.ES, 2), "es"),), loads=(shape.selector,),
                            symbol=True))
                    expanded[0] = mir.raising_owned(expanded[0], op)
                    ops.extend(expanded)
                    arguments.clear()
                    continue
                arguments.clear()
            elif op.kind not in (mir.Kind.COPY, mir.Kind.NOTHING) and not (
                op.op in (ir.Operation.MOVE, ir.Operation.BINARY, ir.Operation.UNARY, ir.Operation.EXTEND)
                and not op.stores and not op.barrier
                and all(ref.addr is not None and ref.space is not Space.STACK for ref in op.loads)
                and (effects := getattr(getattr(op, "node", None), "effects", None)) is not None
                and effects.uses is not None and effects.defs is not None
                and Register.ESP not in effects.uses | effects.defs
            ):
                arguments.clear()
            ops.append(op)
        blocks.append(replace(block, ops=tuple(ops)))
    from qbopt.frontend import raising_array_bounds
    return raising_array_bounds.proven(replace(body, blocks=tuple(blocks)))
