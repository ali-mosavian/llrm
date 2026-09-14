"""Assign floating LIR values to the target register stack."""

from collections import deque
from dataclasses import replace
from collections import defaultdict

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.model.passes import LIRTransform
from qbopt.objectfile.module import Space


def _integer_loads(body: lir.LirBody, frame) -> lir.LirBody:
    """x87 reads integers from memory, for named values and physical stack slots."""
    from qbopt.backend.lower import Unlowered

    blocks = []
    for block in body.blocks:
        insns = []
        for one in block.insns:
            what = one.what
            if (
                what is not None
                and what.op is ir.Operation.FLOAT_LOAD
                and what.name == "fild"
                and len(what.sources) == len(what.dests) == 1
                and isinstance(what.sources[0], (ir.Held, ir.Imm))
                and what.sources[0].width in (2, 4)
                and isinstance(what.dests[0], (ir.Held, ir.St))
            ):
                if isinstance(what.sources[0], ir.Imm) and what.sources[0].value in (0, 1):
                    insns.append(
                        replace(
                            one, what=replace(what, name="fldz" if what.sources[0].value == 0 else "fld1", sources=())
                        )
                    )
                    continue
                if frame is None:
                    raise Unlowered("integer-to-floating conversion requires an owned frame")
                value, destination = what.sources[0], what.dests[0]
                key = destination.value if isinstance(destination, ir.Held) else ("integer-load", one.at)
                cell = frame.cell(key, value.width)
                uses = (value.value,) if isinstance(value, ir.Held) else ()
                insns.append(
                    lir.Insn(
                        one.at, (one.at, one.at), ir.Semantics(ir.Operation.MOVE, "mov", (cell,), (value,)), (), uses
                    )
                )
                one = replace(
                    one, what=replace(what, sources=(cell,)), uses=tuple(arg for arg in one.uses if arg not in uses)
                )
            insns.append(one)
        blocks.append(replace(block, insns=tuple(insns)))
    return replace(body, blocks=tuple(blocks))


def _integer_stores(body: lir.LirBody, frame, basic_semantics: bool) -> lir.LirBody:
    """Materialize integer conversions, including runtime results in physical ST0."""
    from qbopt.backend.lower import Unlowered

    integer_readers = set(body.pins)
    for block in body.blocks:
        integer_readers.update(value for phi in block.phis for _, value in phi.incoming)
        for one in block.insns:
            integer_readers.update(one.uses)
            if one.what is not None:
                integer_readers.update(arg.value for arg in one.what.sources if isinstance(arg, ir.Held))
    unknown_readers = any(one.what is None or one.what.op is ir.Operation.BARRIER for one in body.insns)
    blocks = []
    for block in body.blocks:
        insns = []
        for one in block.insns:
            what = one.what
            if (
                what is None
                or what.op is not ir.Operation.FLOAT_STORE
                or what.name != "fistp"
                or len(what.sources) != 1
                or len(what.dests) != 1
                or not isinstance(what.dests[0], ir.Held)
                or what.dests[0].width not in (2, 4)
            ):
                insns.append(one)
                continue
            if frame is None:
                raise Unlowered("floating-to-integer conversion requires an owned frame")
            result = what.dests[0]
            cell = frame.cell(("integer-conversion", result.value), result.width)
            wait = lir.Insn(one.at, (one.at, one.at), ir.Semantics(ir.Operation.NOTHING, "wait", (), ()), (), ())
            store = replace(
                one,
                what=replace(what, dests=(cell,)),
                defines=tuple(value for value in one.defines if value != result.value),
                widths=tuple((value, width) for value, width in one.widths if value != result.value),
            )
            insns.extend((wait, store, wait) if basic_semantics else (store,))
            if unknown_readers or result.value in integer_readers:
                insns.append(
                    lir.Insn(
                        one.at,
                        (one.at, one.at),
                        ir.Semantics(ir.Operation.MOVE, "mov", (result,), (cell,)),
                        (result.value,),
                        (),
                        widths=((result.value, result.width),),
                    )
                )
        blocks.append(replace(block, insns=tuple(insns)))
    return replace(body, blocks=tuple(blocks))


_ARITHMETIC = ("fadd", "fsub", "fmul", "fdiv")
# `left op right` with the operands' places swapped: `st(i) := st(0) - st(i)` is `fsubr st(i),st(0)`.
_REVERSED = {"fadd": "fadd", "fmul": "fmul", "fsub": "fsubr", "fdiv": "fdivr"}


def _floating(arg) -> bool:
    return isinstance(arg, ir.Held) and arg.width == 10


def _loads_memory(what: ir.Semantics | None) -> bool:
    return (
        what is not None
        and what.op is ir.Operation.FLOAT_LOAD
        and what.name in ("fld", "fild")
        and len(what.sources) == 1
        and isinstance(what.sources[0], ir.Mem)
        and len(what.dests) == 1
        and isinstance(what.dests[0], ir.Held)
    )


def _two_values(what: ir.Semantics | None) -> "tuple[str, ir.Held, ir.Held] | None":
    """Arithmetic on two floating values, as `result := left name right` with name in _ARITHMETIC."""
    if (
        what is None
        or what.op not in (ir.Operation.FLOAT_ARITH, ir.Operation.FLOAT_ARITH_POP)
        or len(what.sources) != 2
        or not all(map(_floating, what.sources))
    ):
        return None
    name = what.name[:-1] if what.op is ir.Operation.FLOAT_ARITH_POP else what.name
    left, right = what.sources
    if name in ("fsubr", "fdivr"):
        return name[:-1], right, left
    return (name, left, right) if name in _ARITHMETIC else None


def _memory_name(name: str, cell_is_left: bool, load: lir.Insn) -> str | None:
    """The instruction computing `name` with one operand read from `load`'s cell, if x87 has one."""
    from qbopt.backend import select

    name = _REVERSED[name] if cell_is_left else name
    if load.what.name == "fild":
        name = "fi" + name[1:]
    return name if select.float_memory(name, load.what.sources[0]) is not None else None


def _stable(cell: ir.Mem) -> bool:
    """Whether the cell's address is made of SSA values, so every read of it names the same bytes."""
    if cell.addr is None or cell.addr.space is Space.FAR and cell.selector is None:
        return False
    return cell.base is not None or cell.index is not None or cell.through in (Register.NONE, Register.BP)


def _reached(cell: ir.Mem):
    # regions reads a based address as reaching its whole region.
    indexed = cell.base is not None or cell.index is not None
    return replace(cell.addr, base=cell.addr.base or cell.through or Register.SI) if indexed else cell.addr


def _may_write(one: lir.Insn, cell: ir.Mem) -> bool:
    from qbopt.analysis import regions

    what = one.what
    if what is None or what.op is ir.Operation.FILL:
        return True
    if any(
        isinstance(dest, ir.Mem)
        and (dest.addr is None or regions.addresses(_reached(dest), dest.width, _reached(cell), cell.width))
        for dest in what.dests
    ):
        return True
    address = {arg.value for arg in (cell.base, cell.index, cell.selector) if arg is not None}
    return not address.isdisjoint(one.defines)


def _may_raise(one: lir.Insn) -> bool:
    """Whether the instruction can raise anything but a load's invalid operand.

    Loads are left out: two reads that can each raise only invalid operation
    raise the same thing in either order.
    """
    what = one.what
    if what is None:
        return True
    match what.op:
        case ir.Operation.FLOAT_ARITH | ir.Operation.FLOAT_ARITH_POP | ir.Operation.FLOAT_UNARY | ir.Operation.DIVIDE:
            return True
        case ir.Operation.FLOAT_STORE:
            extended = all(isinstance(dest, ir.Mem) and dest.width == 10 for dest in what.dests)
            return not (what.name == "fstp" and extended)
        case ir.Operation.NOTHING:
            return what.name == "wait"
    return False


def _quiet(sequence: list[lir.Insn], position: int, cell: ir.Mem) -> bool:
    """Whether an x87 store wrote the cell last, so it holds no signalling NaN."""
    for one in reversed(sequence[:position]):
        what = one.what
        if what.op is ir.Operation.FLOAT_STORE and what.name in ("fstp", "fst") and what.dests == (cell,):
            return True
        if _may_write(one, cell):
            return False
    return False


def _rereadable(sequence: list[lir.Insn], position: int, reads: deque) -> bool:
    """Whether the load at `position` may be read again by each reader instead of held on the stack.

    GCC's memory equivalence: a value that is a cell nothing writes before its
    last reader is that cell. Its first read moves to its first reader, so
    nothing that can raise may come between unless the read cannot.
    """
    load = sequence[position]
    if not _loads_memory(load.what) or load.delivers or not reads:
        return False
    cell = load.what.sources[0]
    if not _stable(cell):
        return False
    quiet = load.what.name == "fild" or cell.width == 10 or None
    for step in range(position + 1, reads[-1]):
        one = sequence[step]
        if _may_write(one, cell):
            return False
        if step < reads[0] and _may_raise(one):
            if quiet is None:
                quiet = _quiet(sequence, position, cell)
            if not quiet:
                return False
    return True


def _region(blocks: tuple[lir.LirBlock, ...], index: int, offset: int, continues: set[int]):
    """The instructions from here to the region's end, and where each floating value is read among them."""
    from qbopt.backend.floatregions import boundary

    sequence, reads = [], defaultdict(deque)
    while True:
        for instruction in blocks[index].insns[offset:]:
            if boundary(instruction):
                return sequence, reads
            for arg in instruction.what.sources:
                if _floating(arg):
                    reads[arg.value].append(len(sequence))
            sequence.append(instruction)
        if index not in continues:
            return sequence, reads
        index, offset = index + 1, 0


class _Stack:
    """The x87 register stack across one region, after LLVM's X86FloatingPoint.

    An operand an instruction consumes dies there, and the result takes its
    slot. A value loaded from a cell is not held at all while the cell stays
    unwritten: each reader takes the cell as its memory operand or reloads it.
    """

    def __init__(self, frame, floating: set[int]):
        self.frame, self.floating = frame, floating
        self.values: list[int] = []  # top first
        self.home: dict[int, lir.Insn] = {}  # the load that reads a value again
        self.defined: dict[int, int] = {}
        self.sequence: list[lir.Insn] = []
        self.reads: defaultdict = defaultdict(deque)
        self.here = -1
        self.out: list[lir.Insn] = []
        self.one: lir.Insn | None = None
        self.keep: set[int] = set()
        self.vacated: set[int] = set()

    def region(self, sequence: list[lir.Insn], reads: defaultdict) -> None:
        self.sequence, self.reads, self.here = sequence, reads, -1
        self.home.clear()

    def pending(self, value: int) -> deque:
        """Where the value is read after this instruction."""
        reads = self.reads[value]
        while reads and reads[0] <= self.here:
            reads.popleft()
        return reads

    def survives(self, value: int) -> bool:
        """Whether a stack copy of the value is read after this instruction."""
        if value not in self.home:
            return bool(self.pending(value))
        return not all(self._reads_cell(value, step) for step in self.pending(value))

    def _reads_cell(self, value: int, step: int) -> bool:
        found = _two_values(self.sequence[step].what)
        if found is None:
            return False
        name, left, right = found
        return left != right and _memory_name(name, left.value == value, self.home[value]) is not None

    def insert(self, what: ir.Semantics) -> None:
        at = self.one.at
        self.out.append(lir.Insn(at, (at, at), what, (), ()))

    def emit(self, what: ir.Semantics, *, uses=(), widths=(), **changes) -> None:
        one = self.one
        self.out.append(
            replace(
                one,
                what=what,
                uses=tuple(dict.fromkeys(value for value in (*one.uses, *uses) if value not in self.floating)),
                defines=tuple(value for value in one.defines if value not in self.floating),
                widths=tuple(dict.fromkeys(pair for pair in (*one.widths, *widths) if pair[0] not in self.floating)),
                **changes,
            )
        )

    def vacate(self) -> None:
        """Nothing is emitted here, and the bytes are still accounted for."""
        one = self.one
        if one.covers and one.covers[0] != one.covers[1]:
            nothing = ir.Semantics(ir.Operation.NOTHING, "", (), ())
            self.out.append(replace(one, what=nothing, uses=(), defines=(), widths=(), requires=()))
            self.vacated.add(id(self.out[-1]))

    def exchange(self, slot: int) -> None:
        if slot:
            operands = ir.St(0), ir.St(slot)
            self.insert(ir.Semantics(ir.Operation.EXCHANGE, "fxch", operands, operands))
            self.values[0], self.values[slot] = self.values[slot], self.values[0]

    def room(self, count: int) -> None:
        from qbopt.backend.lower import Unlowered

        while len(self.values) + count > 8:
            if self.frame is None:
                raise Unlowered("floating spill requires an owned frame")
            victim = max(
                (slot for slot, value in enumerate(self.values) if value not in self.keep),
                key=lambda slot: (self.pending(self.values[slot]) or [float("inf")])[0],
                default=None,
            )
            if victim is None:
                raise Unlowered("floating instruction requires too many stack operands")
            self.exchange(victim)
            value = self.values.pop(0)
            cell = self.frame.cell(("floating", value), 10)
            self.insert(ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (ir.St(0),)))
            at = self.one.at
            load = ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (ir.Held(value, 10),), (cell,))
            self.home[value] = lir.Insn(at, (at, at), load, (), ())

    def duplicate(self, value: int) -> None:
        self.room(1)
        self.insert(ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (ir.St(0),), (ir.St(self.values.index(value)),)))
        self.values.insert(0, value)

    def materialize(self, value: int) -> None:
        from qbopt.backend.lower import Unlowered

        if value not in self.home:
            raise Unlowered("floating stack input is unavailable")
        self.room(1)
        load, at = self.home[value], self.one.at
        self.out.append(
            replace(
                load,
                at=at,
                covers=(at, at),
                what=replace(load.what, dests=(ir.St(0),)),
                defines=(),
                uses=tuple(one for one in load.uses if one not in self.floating),
                widths=tuple(pair for pair in load.widths if pair[0] not in self.floating),
                spread=(),
                symbol=None,
            )
        )
        self.values.insert(0, value)

    def top(self, value: int) -> None:
        if value not in self.values:
            self.materialize(value)
        self.exchange(self.values.index(value))

    def allocate(self, one: lir.Insn) -> None:
        from qbopt.backend.lower import Unlowered

        if self.sequence[self.here] is not one:
            raise Unlowered("floating region positions disagree")
        self.one, what = one, one.what
        operands = [arg.value for arg in what.sources if _floating(arg)]
        results = [arg.value for arg in what.dests if _floating(arg)]
        self.keep = set(operands)
        if len(results) > 1 or any(result in self.values for result in results):
            raise Unlowered("floating stack result is not a fresh value")
        for result in results:
            self.defined[result] = self.here
        two = _two_values(what)
        if two is not None and results:
            self.arithmetic(*two, results[0])
        elif what.op is ir.Operation.FLOAT_LOAD and not operands and results:
            if _rereadable(self.sequence, self.here, self.pending(results[0])):
                self.home[results[0]] = one
                self.vacate()
            else:
                self.room(1)
                self.emit(replace(what, dests=(ir.St(0),)))
                self.values.insert(0, results[0])
        elif what.op is ir.Operation.FLOAT_LOAD and len(operands) == 1 and results:
            self.copy(operands[0], results[0])
        elif what.op is ir.Operation.FLOAT_STORE and len(operands) == 1 and not results:
            self.store(operands[0])
        elif what.op is ir.Operation.FLOAT_UNARY and len(operands) == 1 and results:
            self.consume(operands[0], replace(what, dests=(ir.St(0),), sources=(ir.St(0),)), results[0])
        elif what.op is ir.Operation.FLOAT_ARITH and operands and _floating(what.sources[0]) and results:
            kept = what.sources[0].value
            self.consume(kept, replace(what, dests=(ir.St(0),), sources=(ir.St(0), *what.sources[1:])), results[0])
        else:
            raise Unlowered("floating instruction has no allocation rule")

    def copy(self, source: int, result: int) -> None:
        if source in self.values and not self.survives(source):
            # GCC's move_for_stack_reg: a source dying here is renamed, not copied.
            self.values[self.values.index(source)] = result
            self.vacate()
            return
        if source not in self.values:
            self.materialize(source)
            self.values[0] = result
            self.vacate()
            return
        self.room(1)
        self.emit(replace(self.one.what, dests=(ir.St(0),), sources=(ir.St(self.values.index(source)),)))
        self.values.insert(0, result)

    def store(self, source: int) -> None:
        what = self.one.what
        self.top(source)
        name = "fstp" if what.name == "fst" else what.name
        if self.survives(source):
            if name == "fstp" and all(isinstance(dest, ir.Mem) and dest.width in (4, 8) for dest in what.dests):
                self.emit(replace(what, name="fst", sources=(ir.St(0),)))
                return
            self.duplicate(source)
        self.emit(replace(what, name=name, sources=(ir.St(0),)))
        self.values.pop(0)

    def consume(self, source: int, what: ir.Semantics, result: int) -> None:
        """An instruction replacing the top with its result."""
        self.top(source)
        if self.survives(source):
            self.duplicate(source)
        self.emit(what)
        self.values[0] = result

    def arithmetic(self, name: str, left: ir.Held, right: ir.Held, result: int) -> None:
        left, right = left.value, right.value
        cells = sorted(
            (
                (cell, kept)
                for cell, kept in ((right, left), (left, right))
                if cell != kept
                and cell not in self.values
                and cell in self.home
                and _memory_name(name, cell == left, self.home[cell]) is not None
            ),
            # Both in their cells: the later one is the memory operand, so the loads keep their order.
            key=lambda pair: -self.defined.get(pair[0], -1),
        )
        if cells:
            cell, kept = cells[0]
            load, covers = self.home[cell], self.one.covers
            operation = _memory_name(name, cell == left, load)
            self.top(kept)
            if self.survives(kept):
                self.duplicate(kept)
            self.emit(
                ir.Semantics(ir.Operation.FLOAT_ARITH, operation, (ir.St(0),), (ir.St(0), load.what.sources[0])),
                uses=load.uses,
                widths=load.widths,
                requires=tuple(dict.fromkeys((*load.requires, *self.one.requires))),
                # The operand is another instruction's: its fixup is bound by address, not by this one's record.
                symbol=False if covers and covers[0] != covers[1] else self.one.symbol,
            )
            self.values[0] = result
            return
        for value in sorted({left, right} - set(self.values), key=lambda value: self.defined.get(value, -1)):
            self.materialize(value)
        dies_left, dies_right = not self.survives(left), not self.survives(right)
        if left == right:
            self.exchange(self.values.index(left))
            slot = 0
            if not dies_left:
                self.duplicate(left)
                # Two copies of one value: the result takes the slot that leaves the sooner read on top.
                later, product = self.pending(left), self.pending(result)
                slot = 1 if later and product and later[0] < product[0] else 0
            operands = (ir.St(slot), ir.St(1 - slot)) if not dies_left else (ir.St(0), ir.St(0))
            self.emit(ir.Semantics(ir.Operation.FLOAT_ARITH, name, (ir.St(slot),), operands))
            self.values[slot] = result
            return
        # LLVM's handleTwoArgFP: a dying operand goes on top so the result can overwrite it.
        if self.values[0] not in (left, right):
            if dies_left or dies_right:
                self.exchange(self.values.index(left if dies_left else right))
            else:
                self.duplicate(left)
                dies_left = True
        elif not dies_left and not dies_right:
            self.duplicate(left)
            dies_left = True
        forward = self.values[0] == left
        other = self.values.index(right if forward else left)
        if (forward and not dies_right) or (not forward and not dies_left):
            operation = name if forward else _REVERSED[name]
            self.emit(ir.Semantics(ir.Operation.FLOAT_ARITH, operation, (ir.St(0),), (ir.St(0), ir.St(other))))
            self.values[0] = result
        elif dies_left and dies_right:
            operation = (_REVERSED[name] if forward else name) + "p"
            self.emit(
                ir.Semantics(ir.Operation.FLOAT_ARITH_POP, operation, (ir.St(other),), (ir.St(other), ir.St(0)))
            )
            self.values[other] = result
            self.values.pop(0)
        else:
            operation = _REVERSED[name] if forward else name
            self.emit(ir.Semantics(ir.Operation.FLOAT_ARITH, operation, (ir.St(other),), (ir.St(other), ir.St(0))))
            self.values[other] = result


def allocated(body: lir.LirBody, frame=None, *, basic_semantics: bool = True) -> lir.LirBody:
    from qbopt.backend.lower import Unlowered

    body = _integer_stores(_integer_loads(body, frame), frame, basic_semantics)
    floating = {
        arg.value
        for block in body.blocks
        for one in block.insns
        if one.what
        for arg in (*one.what.sources, *one.what.dests)
        if isinstance(arg, ir.Held) and arg.width == 10
    }
    if not floating:
        return body
    predecessors = {block.at: set() for block in body.blocks}
    for block in body.blocks:
        for successor in block.succ:
            if successor in predecessors:
                predecessors[successor].add(block.at)
    order = tuple(block.at for block in body.blocks)
    next_blocks = {
        block.at: block.succ[0]
        for block in body.blocks
        if len(block.succ) == 1 and block.succ[0] != body.entry and predecessors.get(block.succ[0]) == {block.at}
    }
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
        index
        for index, (block, following) in enumerate(zip(body.blocks, body.blocks[1:]))
        if next_blocks.get(block.at) == following.at
    }
    from qbopt.backend.floatregions import boundary
    from qbopt.backend.floatregions import bridged

    # A region is keyed by instruction position: a phi is defined at -1 and
    # read at its predecessor's end.
    regions, region = {}, 0
    for index, block in enumerate(body.blocks):
        if index - 1 not in continues:
            region += 1
        regions[block.at, -1] = region
        for position, one in enumerate(block.insns):
            region += boundary(one)
            regions[block.at, position] = region
        regions[block.at, len(block.insns)] = region
    body = bridged(body, regions, frame)
    if len(body.blocks) != len(order):
        # Splitting critical edges changes the regions and their stack lifetimes.
        by_at = {block.at: block for block in body.blocks}
        return allocated(
            replace(
                body,
                blocks=tuple(by_at[at] for at in order)
                + tuple(block for block in body.blocks if block.at not in order),
            ),
            frame,
        )
    floating = {
        arg.value
        for block in body.blocks
        for one in block.insns
        if one.what
        for arg in (*one.what.sources, *one.what.dests)
        if isinstance(arg, ir.Held) and arg.width == 10
    }
    stack = _Stack(frame, floating)
    blocks = []
    for index, block in enumerate(body.blocks):
        if index - 1 not in continues:
            stack.region(*_region(body.blocks, index, 0, continues))
        # By identity, so only while every instruction marked is still in `out`.
        stack.out, stack.vacated = [], set()
        for position, one in enumerate(block.insns):
            stack.here += 1
            what = one.what
            if what is None or not any(_floating(arg) for arg in (*what.sources, *what.dests)):
                if floating.intersection((*one.uses, *one.defines)):
                    raise Unlowered("floating value used by an unmodelled instruction")
                if boundary(one):
                    # bridged() gave every value read beyond here its own cell.
                    if stack.values:
                        raise Unlowered("floating stack crosses an unmodelled instruction")
                    stack.region(*_region(body.blocks, index, position + 1, continues))
                stack.out.append(one)
                continue
            stack.allocate(one)
        if stack.values and index not in continues:
            raise Unlowered("floating stack live-out requires cross-block allocation")
        blocks.append(replace(block, insns=tuple(lir.without(stack.out, lambda one: id(one) in stack.vacated))))
    allocated_blocks = {block.at: block for block in blocks}
    return replace(body, blocks=tuple(allocated_blocks[at] for at in order))


class FloatAlloc(LIRTransform):
    name = "floatalloc"

    def __init__(self, frame=None, *, basic_semantics: bool = True):
        self.frame = frame
        self.basic_semantics = basic_semantics

    def transform(self, body):
        return allocated(body, self.frame, basic_semantics=self.basic_semantics)
