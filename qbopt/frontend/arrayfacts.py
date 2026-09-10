"""Must-facts for whole array pointers across every CFG path.

An in-bounds access establishes disjointness before its store is interpreted.
Unknown branches meet equal facts; unknown effects invalidate allocation facts.
No original-register location participates in this analysis.
"""

from dataclasses import dataclass, field, replace
from math import prod

from qbopt.analysis import consts, loops
from qbopt.model import mir
from qbopt.objectfile.module import Addr, Space


@dataclass(frozen=True)
class Pointer:
    allocation: mir.Symbol
    offset: int
    generation: tuple[int, int]


@dataclass(frozen=True)
class Allocation:
    extent: int
    descriptor_size: int
    generation: tuple[int, int]


type Fact = int | Pointer


@dataclass
class State:
    values: dict[mir.Value, tuple[Fact, int]] = field(default_factory=dict)
    memory: dict[tuple[Addr | Pointer, int], Fact] = field(default_factory=dict)
    allocations: dict[mir.Symbol, Allocation] = field(default_factory=dict)

    def copy(self):
        return State(dict(self.values), dict(self.memory), dict(self.allocations))

    def forget_memory(self):
        self.memory.clear()
        self.allocations.clear()


def _meet(states):
    first, *rest = states
    def common(name):
        return {key: value for key, value in getattr(first, name).items()
                if all(getattr(other, name).get(key) == value for other in rest)}
    return State(common("values"), common("memory"), common("allocations"))


def _address(ref, state):
    if ref.pointer:
        if ref.base_width != 4 or ref.addr is not None or ref.segment is not None:
            return None
        pointer = _read(mir.Held(ref.base, ref.base_width), state)
        if (isinstance(pointer, Pointer) and pointer.allocation in state.allocations
            and pointer.generation == state.allocations[pointer.allocation].generation
            and ref.width > 0 and 0 <= pointer.offset <= state.allocations[pointer.allocation].extent - ref.width):
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
            return consts.masked(fact, width) if isinstance(fact, int) else fact if width == 4 else None
        case mir.Cell(ref=ref):
            return state.memory.get((_address(ref, state), ref.width))
    return None


def _result(op, args):
    match op.kind, args:
        case (mir.Kind.COPY | mir.Kind.LOAD | mir.Kind.STORE), [value]:
            return value
        case mir.Kind.PTR_OFFSET, [Pointer(offset=offset) as pointer, int() as delta]:
            offset += ((delta & 0xffffffff) ^ 0x80000000) - 0x80000000
            return replace(pointer, offset=offset) if -0x80000000 <= offset <= 0x7fffffff else None
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
    return None


def _overlap(left, width, right, size):
    match left, right:
        case Addr(), Addr():
            return (left.space is right.space and left.index == right.index
                    and left.disp < right.disp + size and right.disp < left.disp + width)
        case Pointer(), Pointer():
            return (left.allocation == right.allocation and left.generation == right.generation
                    and left.offset < right.offset + size and right.offset < left.offset + width)
    return False


def _transfer(block, arriving, cyclic):
    state = arriving.copy()
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
            if isinstance(result, int):
                state.values[value.value] = consts.masked(result, value.width), value.width
            elif value.width == 4:
                state.values[value.value] = result, value.width
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
            if result is not None:
                state.memory[address, ref.width] = consts.masked(result, ref.width) if isinstance(result, int) else result
    return state, checked


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
    outgoing = {}
    entries = {}
    while True:
        changed = False
        for block in body.blocks:
            incoming = [outgoing[parent] for parent in predecessors[block.at] if parent in outgoing]
            if block.at == body.entry:
                incoming.append(State())
            if not incoming:
                continue
            limit -= max(1, len(block.ops))
            if limit < 0:
                return body
            arriving = _meet(incoming)
            for phi in block.phis:
                facts = [outgoing[parent].values.get(value) for parent, value in phi.incoming.items() if parent in outgoing]
                if facts and facts[0] is not None and all(fact == facts[0] for fact in facts):
                    arriving.values[phi.result] = facts[0]
                else:
                    arriving.values.pop(phi.result, None)
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
