"""Must-facts for whole array pointers across every CFG path.

An in-bounds access establishes disjointness before its store is interpreted.
Unknown branches join numeric ranges; unknown effects invalidate allocation facts.
Loop-header widening and edge refinement establish inductive bounds without
enumerating iterations. Only converged states annotate accesses.
No original-register location participates in this analysis.
"""

from dataclasses import dataclass, field, replace
from math import prod

from qbopt.analysis import consts, loops, ranges
from qbopt.model import mir
from qbopt.objectfile.module import Addr, Space


@dataclass(frozen=True)
class Pointer:
    allocation: mir.Symbol
    offset: int | ranges.Interval
    generation: tuple[int, int]


@dataclass(frozen=True)
class Allocation:
    extent: int
    descriptor_size: int
    generation: tuple[int, int]


type Fact = int | ranges.Interval | Pointer


@dataclass
class State:
    values: dict[mir.Value, tuple[Fact, int]] = field(default_factory=dict)
    memory: dict[tuple[Addr | Pointer, int], Fact] = field(default_factory=dict)
    allocations: dict[mir.Symbol, Allocation] = field(default_factory=dict)
    bindings: dict[tuple[Addr | Pointer, int], mir.Value] = field(default_factory=dict)

    def copy(self):
        return State(dict(self.values), dict(self.memory), dict(self.allocations), dict(self.bindings))

    def forget_memory(self):
        self.memory.clear()
        self.allocations.clear()
        self.bindings.clear()


def _span(fact, width):
    if isinstance(fact, ranges.Interval):
        return fact if fact.width == width else None
    if isinstance(fact, int):
        sign = 1 << (width * 8 - 1)
        number = ((fact & (2 * sign - 1)) ^ sign) - sign
        return ranges.Interval(number, number, width)
    return None


def _fitted(fact, width):
    if isinstance(fact, int):
        return consts.masked(fact, width)
    if isinstance(fact, ranges.Interval):
        if fact.width < width:
            return None
        sign = 1 << (width * 8 - 1)
        if -sign <= fact.low <= fact.high < sign:
            return consts.masked(fact.low, width) if fact.low == fact.high else replace(fact, width=width)
        return None
    return fact if isinstance(fact, Pointer) and width == 4 else None


def _joined(facts, width):
    if not facts or any(fact is None for fact in facts):
        return None
    if all(fact == facts[0] for fact in facts):
        return facts[0]
    spans = [_span(fact, width) for fact in facts]
    if all(span is not None for span in spans):
        return ranges.Interval(min(span.low for span in spans), max(span.high for span in spans), width)
    first = facts[0]
    if isinstance(first, Pointer) and all(isinstance(fact, Pointer) and fact.allocation == first.allocation
                                         and fact.generation == first.generation for fact in facts):
        offset = _joined([fact.offset for fact in facts], 4)
        return replace(first, offset=offset)
    return None


def _joined_values(facts):
    if not facts or any(fact is None for fact in facts) or len({fact[1] for fact in facts}) != 1:
        return None
    width = facts[0][1]
    joined = _joined([fact[0] for fact in facts], width)
    return (joined, width) if joined is not None else None


def _meet(states):
    first, *rest = states
    def common(name):
        return {key: value for key, value in getattr(first, name).items()
                if all(getattr(other, name).get(key) == value for other in rest)}
    values = {key: joined for key in first.values
              if (joined := _joined_values([state.values.get(key) for state in states])) is not None}
    memory = {key: joined for key in first.memory
              if (joined := _joined([state.memory.get(key) for state in states], key[1])) is not None}
    return State(values, memory, common("allocations"))


def _address(ref, state):
    if ref.pointer:
        if ref.base_width != 4 or ref.addr is not None or ref.segment is not None:
            return None
        pointer = _read(mir.Held(ref.base, ref.base_width), state)
        if (isinstance(pointer, Pointer) and pointer.allocation in state.allocations
            and pointer.generation == state.allocations[pointer.allocation].generation
            and ref.width > 0 and 0 <= _span(pointer.offset, 4).low
            and _span(pointer.offset, 4).high <= state.allocations[pointer.allocation].extent - ref.width):
            return pointer
        return None
    ref = mir._symbolic_ref(ref)
    return (ref.addr if ref.addr is not None and ref.addr.space in (Space.SEGMENT, Space.FRAME)
            and ref.base is None and ref.segment is None
            and ref.addr == Addr(ref.addr.space, ref.addr.disp, ref.addr.index) else None)


def _read(arg, state):
    match arg:
        case mir.Const(n=number, width=width):
            return consts.masked(number, width)
        case mir.Held(value=value, width=width):
            known = state.values.get(value)
            if known is None or known[1] < width:
                return None
            fact = known[0]
            return _fitted(fact, width)
        case mir.Cell(ref=ref):
            address = _address(ref, state)
            if isinstance(address, Pointer) and isinstance(address.offset, ranges.Interval):
                return None
            return state.memory.get((address, ref.width))
    return None


def _result(op, args):
    if op.kind is mir.Kind.COPY and len(op.args) == len(op.results) == 1:
        if isinstance(op.results[0], mir.Held) and op.results[0].width != getattr(op.args[0], "width", None):
            return None
    if op.kind is mir.Kind.XOR and len(op.args) == 2 and op.args[0] == op.args[1]:
        return 0
    match op.kind, args:
        case (mir.Kind.COPY | mir.Kind.LOAD | mir.Kind.STORE), [value]:
            return value
        case mir.Kind.PTR_OFFSET, [Pointer(offset=offset) as pointer, (int() | ranges.Interval()) as delta]:
            left, right = _span(offset, 4), _span(delta, 4)
            if left is None or right is None:
                return None
            offset = _fitted(ranges.Interval(left.low + right.low, left.high + right.high, 4), 4)
            return replace(pointer, offset=offset) if offset is not None else None
        case mir.Kind.SIGN_EXTEND, [int() as value]:
            sign = 1 << (op.args[0].width * 8 - 1)
            return (value ^ sign) - sign
        case mir.Kind.INCREMENT, [int() as value]:
            return value + 1
        case mir.Kind.DECREMENT, [int() as value]:
            return value - 1
        case kind, [int() as left, int() as right] if kind in consts.ARITH:
            try:
                return consts.ARITH[kind](left, right)
            except (ZeroDivisionError, ValueError):
                return None
    if any(isinstance(arg, ranges.Interval) for arg in args):
        known = {operand.value: span for operand, fact in zip(op.args, args)
                 if isinstance(operand, mir.Held) and (span := _span(fact, operand.width)) is not None}
        return ranges._computed(op, known, {})
    return None


def _overlap(left, width, right, size):
    match left, right:
        case Addr(), Addr():
            return (left.space is right.space and left.index == right.index
                    and left.disp < right.disp + size and right.disp < left.disp + width)
        case Pointer(), Pointer():
            return (left.allocation == right.allocation and left.generation == right.generation
                    and _span(left.offset, 4).low < _span(right.offset, 4).high + size
                    and _span(right.offset, 4).low < _span(left.offset, 4).high + width)
    return False


def _transfer(block, arriving, cyclic):
    state = arriving.copy()
    state.bindings.clear()
    checked = {}
    for index, op in enumerate(block.ops):
        if op.kind is mir.Kind.CALL or op.barrier or op.floating is not None or op.kind is mir.Kind.FCHECK:
            state.forget_memory()
            for value in op.defines:
                state.values.pop(value, None)
            request = op.array
            if (request is not None and not request.replaces and op.memory_values and not cyclic
                and request.element_width > 0 and all(high >= low for low, high in request.bounds)):
                extent = request.element_width * prod(high - low + 1 for low, high in request.bounds)
                if 0 < extent < 1 << 31:
                    descriptor = request.descriptor
                    generation = block.at, index
                    state.allocations[descriptor] = Allocation(extent, 14 + 4 * len(request.bounds), generation)
                    state.memory.update(((address, ref.width), value.n) for ref, value in op.memory_values
                                        if (address := _address(ref, state)) is not None)
                    base = Addr(descriptor.space, descriptor.offset + descriptor.addend, descriptor.index)
                    state.memory[base, 4] = Pointer(descriptor, 0, generation)
            continue
        args = [_read(arg, state) for arg in op.args]
        result = _result(op, args)
        for value in op.defines:
            state.values.pop(value, None)
        held = [arg for arg in op.results if isinstance(arg, mir.Held)]
        if len(held) == 1 and result is not None:
            value, = held
            fitted = _fitted(result, value.width)
            if fitted is not None:
                state.values[value.value] = fitted, value.width
        for ref in (*op.loads, *op.stores):
            address = _address(ref, state)
            if ref.pointer and isinstance(address, Pointer):
                checked[index, ref] = address.allocation
        for ref in op.stores:
            address = _address(ref, state)
            if address is None:
                state.forget_memory()
                continue
            if isinstance(address, Addr):
                for descriptor in list(state.allocations):
                    base = Addr(descriptor.space, descriptor.offset + descriptor.addend, descriptor.index)
                    if _overlap(address, ref.width, base, state.allocations[descriptor].descriptor_size):
                        # The complete descriptor is protected, not just its pointer word.
                        state.forget_memory()
                        break
            state.memory = {key: value for key, value in state.memory.items()
                            if not _overlap(address, ref.width, *key)}
            state.bindings = {key: value for key, value in state.bindings.items()
                              if not _overlap(address, ref.width, *key)}
            fitted = _fitted(result, ref.width)
            if fitted is not None and not (isinstance(address, Pointer) and isinstance(address.offset, ranges.Interval)):
                state.memory[address, ref.width] = fitted
                if op.kind is mir.Kind.STORE and len(op.args) == 1 and isinstance(op.args[0], mir.Held):
                    state.bindings[address, ref.width] = op.args[0].value
    return state, checked


def _edge(state, block, successor):
    known = {value: span for value, (fact, width) in state.values.items()
             if (span := _span(fact, width)) is not None}
    refined = ranges.on_edge(block, successor, known)
    if refined is None:
        return None
    result = state.copy()
    for value, interval in refined.items():
        result.values[value] = _fitted(interval, interval.width), interval.width
    for key, value in state.bindings.items():
        fact = _read(mir.Held(value, key[1]), result)
        if fact is not None and key in result.memory:
            result.memory[key] = fact
    return result


def _widened(previous, current, width):
    before, after = _span(previous, width), _span(current, width)
    if before is None or after is None:
        return current
    sign = 1 << (width * 8 - 1)
    return _fitted(ranges.Interval(-sign if after.low < before.low else after.low,
                                   sign - 1 if after.high > before.high else after.high, width), width)


def _widen(previous, current):
    for value, (fact, width) in list(current.values.items()):
        before = previous.values.get(value)
        if before is not None and before[1] == width:
            current.values[value] = _widened(before[0], fact, width), width
    for key, fact in list(current.memory.items()):
        if key in previous.memory:
            current.memory[key] = _widened(previous.memory[key], fact, key[1])


def proven(body: mir.MirBody, *, limit: int = 10000) -> mir.MirBody:
    if not any(ref.pointer for block in body.blocks for op in block.ops for ref in (*op.loads, *op.stores)):
        return body
    predecessors = loops.predecessors(body.blocks)
    by_at = {block.at: block for block in body.blocks}
    def revisited(block):
        pending, seen = list(block.succ), set()
        while pending:
            at = pending.pop()
            if at == block.at:
                return True
            if at not in seen and at in by_at:
                seen.add(at)
                pending.extend(by_at[at].succ)
        return False
    cyclic = {block.at for block in body.blocks if any(op.array for op in block.ops) and revisited(block)}
    headers = {loop.header for loop in loops.loops(body.blocks, body.entry)}
    outgoing = {}
    entries = {}
    while True:
        changed = False
        for block in body.blocks:
            incoming_edges = {parent: edge for parent in predecessors[block.at] if parent in outgoing
                              and (edge := _edge(outgoing[parent], by_at[parent], block.at)) is not None}
            incoming = list(incoming_edges.values())
            if block.at == body.entry:
                incoming.append(State())
            if not incoming:
                if block.at in outgoing:
                    del outgoing[block.at]
                    entries.pop(block.at, None)
                    changed = True
                continue
            limit -= max(1, len(block.ops))
            if limit < 0:
                return body
            arriving = _meet(incoming)
            for phi in block.phis:
                facts = [incoming_edges[parent].values.get(value) for parent, value in phi.incoming.items() if parent in incoming_edges]
                if (joined := _joined_values(facts)) is not None:
                    arriving.values[phi.result] = joined
                else:
                    arriving.values.pop(phi.result, None)
            if block.at in headers and block.at in entries:
                _widen(entries[block.at], arriving)
            leaving, _ = _transfer(block, arriving, block.at in cyclic)
            entries[block.at] = arriving
            if leaving != outgoing.get(block.at):
                outgoing[block.at] = leaving
                changed = True
        if not changed:
            break
    blocks = []
    changed = False
    for block in body.blocks:
        if block.at not in entries:
            blocks.append(block)
            continue
        _, checked = _transfer(block, entries[block.at], block.at in cyclic)
        changed |= bool(checked)
        ops = []
        for index, op in enumerate(block.ops):
            def reference(ref):
                allocation = checked.get((index, ref))
                return replace(ref, allocation=allocation) if allocation is not None else ref
            def argument(arg):
                return mir.Cell(reference(arg.ref)) if isinstance(arg, mir.Cell) else arg
            ops.append(replace(op, loads=tuple(map(reference, op.loads)), stores=tuple(map(reference, op.stores)),
                               args=tuple(map(argument, op.args)), results=tuple(map(argument, op.results))))
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks)) if changed else body
