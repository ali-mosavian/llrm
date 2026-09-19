"""Assign floating LIR values to the target register stack."""

from collections import deque
from dataclasses import replace
from collections import defaultdict

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import cpu as targets
from qbopt.objectfile.module import Space
from qbopt.model.passes import LIRTransform


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
                or what.name not in ("fistp", "fisttp")
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


def _reachable_blocks(blocks: tuple[lir.LirBlock, ...], entry: int) -> frozenset[int]:
    """The CFG blocks execution can enter from this body's entry.

    Stack state is a property of an executed edge.  A syntactic predecessor
    in dead code cannot arrive at a join or require an x87 bridge; treating
    it as one made an otherwise straight live edge spill an extended value.
    Keep the dead block for layout and ordinary emission, but exclude its
    edges from the allocator's live control-flow facts.
    """
    at_of = {block.at: block for block in blocks}
    reached, pending = set(), [entry]
    while pending:
        at = pending.pop()
        if at in reached or at not in at_of:
            continue
        reached.add(at)
        pending.extend(at_of[at].succ)
    return frozenset(reached)


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


def _equivalent_loads(sequence: list[lir.Insn]) -> dict[int, int]:
    """Map each repeated stable x87 cell read to the first still-current value.

    Lowering names every ``fld`` with a fresh SSA value.  That is right at
    the MIR/LIR boundary, but loses the fact that two loads of the same stable
    cell, with no intervening write, read one x87 value.  Keep that fact here,
    where the stack allocator can decide whether retaining the value costs
    less than rereading it.  A call, opaque instruction, address redefinition,
    or possibly-aliasing store invalidates the remembered cell through
    ``_may_write``; this is deliberately the same memory proof used by the
    existing memory-operand reuse path.
    """
    available: dict[tuple[str, ir.Mem], int] = {}
    aliases: dict[int, int] = {}
    for one in sequence:
        # A volatile load is observable and may be backed by changing device
        # state.  It must neither be removed nor let an earlier ordinary read
        # stand for a later one.
        if getattr(one.op, "volatile", False):
            available.clear()
            continue
        for key in tuple(available):
            if _may_write(one, key[1]):
                del available[key]
        what = one.what
        if not _loads_memory(what) or one.delivers:
            continue
        cell, result = what.sources[0], what.dests[0]
        # An m80 value is the allocator's extended-precision spill format,
        # and distinct loads can deliberately denote distinct stack values.
        # Rounded scalar cells and integer conversions are ordinary source
        # memory values, so their unchanged reloads are equivalent.
        if cell.width == 10 or not _stable(cell):
            continue
        key = what.name, cell
        if key in available:
            aliases[result.value] = available[key]
        else:
            available[key] = result.value
    return aliases


def _region(blocks: tuple[lir.LirBlock, ...], index: int, offset: int, continues: set[int]):
    """The instructions from here to the region's end, and where each floating value is read among them."""
    from qbopt.backend.floatregions import boundary

    def finished(sequence: list[lir.Insn]):
        aliases, reads = _equivalent_loads(sequence), defaultdict(deque)
        for position, instruction in enumerate(sequence):
            for arg in instruction.what.sources:
                if _floating(arg):
                    reads[aliases.get(arg.value, arg.value)].append(position)
        return sequence, reads, aliases

    sequence = []
    while True:
        for instruction in blocks[index].insns[offset:]:
            if boundary(instruction):
                return finished(sequence)
            sequence.append(instruction)
        if index not in continues:
            return finished(sequence)
        index, offset = index + 1, 0


class _Stack:
    """The x87 register stack across one region, after LLVM's X86FloatingPoint.

    An operand an instruction consumes dies there, and the result takes its
    slot. A value loaded from a cell is not held at all while the cell stays
    unwritten: each reader takes the cell as its memory operand or reloads it.
    """

    def __init__(self, frame, floating: set[int], cpu: targets.Profile, *, retain_homes: bool = True):
        self.frame, self.floating, self.cpu = frame, floating, cpu
        self.retain_homes = retain_homes
        self.values: list[int] = []  # top first
        self.home: dict[int, lir.Insn] = {}  # the load that reads a value again
        self.defined: dict[int, int] = {}
        self.sequence: list[lir.Insn] = []
        self.reads: defaultdict = defaultdict(deque)
        self.aliases: dict[int, int] = {}
        self.here = -1
        self.out: list[lir.Insn] = []
        self.one: lir.Insn | None = None
        self.keep: set[int] = set()
        self.retained: set[int] = set()
        self.vacated: set[int] = set()

    def region(self, sequence: list[lir.Insn], reads: defaultdict, aliases: dict[int, int]) -> None:
        self.sequence, self.reads, self.here = sequence, reads, -1
        self.aliases = aliases
        self.home.clear()
        self.retained.clear()

    def canonical(self, value: int) -> int:
        while value in self.aliases:
            value = self.aliases[value]
        return value

    def semantics(self, what: ir.Semantics) -> ir.Semantics:
        """Replace a repeated direct cell read with its canonical stack value."""
        sources = tuple(
            replace(arg, value=self.canonical(arg.value)) if _floating(arg) else arg for arg in what.sources
        )
        return replace(what, sources=sources) if sources != what.sources else what

    def pending(self, value: int) -> deque:
        """Where the value is read after this instruction."""
        reads = self.reads[self.canonical(value)]
        while reads and reads[0] <= self.here:
            reads.popleft()
        return reads

    def survives(self, value: int) -> bool:
        """Whether a stack copy of the value is read after this instruction."""
        value = self.canonical(value)
        if value in self.retained and value in self.values:
            return bool(self.pending(value))
        if value not in self.home:
            return bool(self.pending(value))
        return not all(self._reads_cell(value, step) for step in self.pending(value))

    def _reads_cell(self, value: int, step: int) -> bool:
        found = _two_values(self.sequence[step].what)
        if found is None:
            return False
        name, left, right = found
        left, right = self.canonical(left.value), self.canonical(right.value)
        return left != right and _memory_name(name, left == value, self.home[value]) is not None

    def arithmetic_costs(self, name: str) -> tuple[int, int, int] | None:
        """The load, register-operation, and memory-operation costs for ``name``."""
        base = {"fadd": "x87_add", "fsub": "x87_add", "fmul": "x87_mul", "fdiv": "x87_div"}[name]
        memory = f"{base}_m"
        if not all(self.cpu.prices(form) for form in ("x87_load", base, memory)):
            return None
        return self.cpu.cost("x87_load"), self.cpu.cost(base), self.cpu.cost(memory)

    def memory_arithmetic(self, name: str, *, preserve_kept: bool = False) -> bool:
        """Whether a cell arithmetic form costs no more than loading it into x87.

        The stack form has one explicit ``fld`` and a register arithmetic;
        the direct form combines those two effects.  If the register operand
        survives, however, the direct form also needs ``fld st(i)`` to keep a
        copy, while loading the dying cell operand lets the result overwrite
        it. A profile without the form-specific prices keeps the historic
        legal memory folding policy only when that preservation copy is not
        required, rather than treating unavailable data as a zero-cost form.
        """
        costs = self.arithmetic_costs(name)
        if costs is None:
            return not preserve_kept
        load, register, memory = costs
        return memory + preserve_kept * load <= load + register

    def retain_home(self, value: int) -> bool:
        """Whether keeping a rereadable home on x87 is cheaper than using the home.

        This compares the complete remaining arithmetic use set.  A self-use
        needs a stack duplicate when the value remains live; an ordinary use
        can instead load its dying peer and overwrite that peer.  Only fully
        priced arithmetic-only use sets are candidates, and two spare stack
        positions are required so the choice cannot manufacture a spill.
        """
        if (
            not self.retain_homes
            or len(self.values) > 6
            or any(retained in self.values and self.pending(retained) for retained in self.retained)
        ):
            return False
        positions = tuple(dict.fromkeys(self.pending(value)))
        if not positions:
            return False
        costs_by_step = []
        for ordinal, step in enumerate(positions):
            found = _two_values(self.sequence[step].what)
            if found is None:
                return False
            name, left, right = found
            left, right = self.canonical(left.value), self.canonical(right.value)
            if value not in (left, right):
                return False
            # Retaining several ordinary operands whose live intervals
            # overlap can make each look profitable alone while forcing
            # exchanges between them.  Start only a retention interval at a
            # self-use, whose unavoidable duplicate is completely costed;
            # later ordinary uses can then consume dying peers around it.
            if ordinal == 0 and left != right:
                return False
            costs = self.arithmetic_costs(name)
            if costs is None:
                return False
            load, register, memory = costs
            if left == right:
                home = load + register
                kept = register + (load if step != positions[-1] else 0)
            else:
                operation = _memory_name(name, left == value, self.home[value])
                home = min(memory, load + register) if operation is not None else load + register
                kept = register
            costs_by_step.append((home, kept))
        load = self.cpu.cost("x87_load")
        home_cost = sum(home for home, _ in costs_by_step)
        retained_cost = load + sum(kept for _, kept in costs_by_step)
        return retained_cost < home_cost

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
        self.one, what = one, self.semantics(one.what)
        operands = [arg.value for arg in what.sources if _floating(arg)]
        results = [arg.value for arg in what.dests if _floating(arg)]
        self.keep = set(operands)
        if _loads_memory(one.what) and len(results) == 1 and self.canonical(results[0]) != results[0]:
            self.vacate()
            return
        if len(results) > 1 or any(result in self.values for result in results):
            raise Unlowered("floating stack result is not a fresh value")
        for result in results:
            self.defined[result] = self.here
        two = _two_values(what)
        if two is not None and results:
            self.arithmetic(*two, results[0])
        elif what.op is ir.Operation.FLOAT_LOAD and not what.name and not what.sources and results:
            # A call's result, which it left in st(0).
            if self.values:
                raise Unlowered("a call's floating result arrives on a stack that is not empty")
            self.values.insert(0, results[0])
            self.vacate()
        elif what.op is ir.Operation.FLOAT_STORE and not what.name and not what.dests and len(operands) == 1:
            # A returned value, left in st(0) for the caller.
            self.top(operands[0])
            if len(self.values) != 1:
                raise Unlowered("a returned float leaves other values on the stack")
            self.values.pop(0)
            self.vacate()
        elif what.op is ir.Operation.COMPARE and len(operands) == 2 and not results:
            self.compare(*operands)
        elif what.op is ir.Operation.FLOAT_LOAD and not operands and results:
            if _rereadable(self.sequence, self.here, self.pending(results[0])):
                self.home[results[0]] = one
                if self.retain_home(results[0]):
                    self.retained.add(results[0])
                    self.room(1)
                    self.emit(replace(what, dests=(ir.St(0),)))
                    self.values.insert(0, results[0])
                else:
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

    def compare(self, left: int, right: int) -> None:
        """`left` against `right`, the answer moved from the status word into the flags."""
        from qbopt.backend import select

        load = self.home.get(right)
        if (
            right != left
            and right not in self.values
            and load is not None
            and load.what.name == "fld"
            and select.float_memory("fcomp", load.what.sources[0]) is not None
        ):
            self.top(left)
            if self.survives(left):
                self.duplicate(left)
            covers = self.one.covers
            self.emit(
                ir.Semantics(ir.Operation.COMPARE, "fcomp", (), (ir.St(0), load.what.sources[0])),
                uses=load.uses,
                widths=load.widths,
                requires=tuple(dict.fromkeys((*load.requires, *self.one.requires))),
                symbol=False if covers and covers[0] != covers[1] else self.one.symbol,
            )
            self.values.pop(0)
        else:
            for value in sorted({left, right} - set(self.values), key=lambda value: self.defined.get(value, -1)):
                self.materialize(value)
            # Left on top and right beneath it, each a copy where it is read again.
            if self.survives(left):
                self.duplicate(left)
            else:
                self.top(left)
            if right == left or self.survives(right):
                self.duplicate(right)
                self.exchange(1)
            elif self.values.index(right) != 1:
                slot = self.values.index(right)
                self.exchange(slot)
                self.exchange(1)
                self.exchange(slot)
            self.emit(ir.Semantics(ir.Operation.COMPARE, "fcompp", (), (ir.St(0), ir.St(1))))
            del self.values[:2]
        at = self.one.at
        status = ir.Semantics(ir.Operation.BARRIER, "fnstsw", (ir.Reg(Register.AX, 2),), ())
        self.out.append(
            lir.Insn(at=at, covers=(at, at), what=status, defines=(), uses=(), clobbers=frozenset({Register.EAX}))
        )
        self.insert(ir.Semantics(ir.Operation.NOTHING, "sahf", (), ()))

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
        if cells and self.memory_arithmetic(name, preserve_kept=self.survives(cells[0][1])):
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
            self.emit(ir.Semantics(ir.Operation.FLOAT_ARITH_POP, operation, (ir.St(other),), (ir.St(other), ir.St(0))))
            self.values[other] = result
            self.values.pop(0)
        else:
            operation = _REVERSED[name] if forward else name
            self.emit(ir.Semantics(ir.Operation.FLOAT_ARITH, operation, (ir.St(other),), (ir.St(other), ir.St(0))))
            self.values[other] = result


def _floating_form(what: ir.Semantics) -> str | None:
    """The profile cost form of an allocated x87 instruction."""
    if what.op is ir.Operation.FLOAT_LOAD:
        return "x87_load"
    if what.op is ir.Operation.EXCHANGE and what.name == "fxch":
        return "x87_exchange"
    if what.op is ir.Operation.FLOAT_STORE:
        return "x87_convert_store" if what.name.startswith("fist") else "x87_store"
    if what.op not in (ir.Operation.FLOAT_ARITH, ir.Operation.FLOAT_ARITH_POP):
        return None
    name = what.name.removeprefix("fi").removesuffix("p").removesuffix("r")
    base = {"fadd": "x87_add", "fsub": "x87_add", "fmul": "x87_mul", "fdiv": "x87_div"}.get(name)
    if base is None:
        return None
    return f"{base}_m" if any(isinstance(arg, ir.Mem) for arg in what.sources) else base


def _allocation_score(body: lir.LirBody, target: targets.Profile) -> tuple[int, int]:
    """Target cost and instruction count after every x87 stack shuffle exists."""
    forms = tuple(form for one in body.insns if one.what and (form := _floating_form(one.what)) is not None)
    if not all(target.prices(form) for form in forms):
        return (sum(1 for _one in body.insns), len(forms))
    return sum(target.cost(form) for form in forms), len(forms)


def _allocate_stack(
    body: lir.LirBody,
    frame,
    floating: set[int],
    target: targets.Profile,
    continues: set[int],
    order: tuple[int, ...],
    *,
    retain_homes: bool,
) -> lir.LirBody:
    """Allocate one complete stack candidate so its real shuffles can be priced."""
    from qbopt.backend.lower import Unlowered
    from qbopt.backend.floatregions import boundary

    stack = _Stack(frame, floating, target, retain_homes=retain_homes)
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


def allocated(
    body: lir.LirBody,
    frame=None,
    *,
    basic_semantics: bool = True,
    cpu: str | targets.Profile = "386",
) -> lir.LirBody:
    target = targets.profile(cpu)
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
    reachable = _reachable_blocks(body.blocks, body.entry)
    predecessors = {block.at: set() for block in body.blocks}
    for block in body.blocks:
        if block.at not in reachable:
            continue
        for successor in block.succ:
            if successor in reachable:
                predecessors[successor].add(block.at)
    order = tuple(block.at for block in body.blocks)
    next_blocks = {
        block.at: block.succ[0]
        for block in body.blocks
        if (
            block.at in reachable
            and len(block.succ) == 1
            and block.succ[0] != body.entry
            and block.succ[0] in reachable
            and predecessors.get(block.succ[0]) == {block.at}
        )
    }
    at_of = {block.at: block for block in body.blocks}
    destinations = set(next_blocks.values())
    roots = [at for at in order if at in reachable and at not in destinations]
    roots += [at for at in order if at not in reachable]
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
    from qbopt.backend.floatregions import bridged
    from qbopt.backend.floatregions import boundary

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
            cpu=target,
        )
    floating = {
        arg.value
        for block in body.blocks
        for one in block.insns
        if one.what
        for arg in (*one.what.sources, *one.what.dests)
        if isinstance(arg, ir.Held) and arg.width == 10
    }
    baseline = _allocate_stack(body, frame, floating, target, continues, order, retain_homes=False)
    retained = _allocate_stack(body, frame, floating, target, continues, order, retain_homes=True)
    selected = min((baseline, retained), key=lambda candidate: _allocation_score(candidate, target))
    return _truncating(selected, frame)


def _truncating(body: lir.LirBody, frame) -> lir.LirBody:
    """`fisttp` as a 387 has it: `fistp` with the control word set to round
    toward zero and put back. After allocation, so the control-word barriers
    split no region.

    The caller's control word and its truncating form are saved once, at
    entry: a callee leaves the control word as it found it, and nothing else
    in the body writes it."""
    from qbopt.backend.lower import Unlowered

    if not any(
        one.what is not None and one.what.op is ir.Operation.FLOAT_STORE and one.what.name == "fisttp"
        for one in body.insns
    ):
        return body
    if frame is None:
        raise Unlowered("rounding toward zero requires an owned frame")
    saved, chop = frame.cell(("control",), 2), frame.cell(("chop",), 2)
    from qbopt.backend.spiller import _next_value

    loaded_id = _next_value(body)
    chopped_id = loaded_id + 1
    loaded = ir.Held(loaded_id, 2)
    chopped = ir.Held(chopped_id, 2)

    def insn(what: ir.Semantics, at: int) -> lir.Insn:
        defines = tuple(arg.value for arg in what.dests if isinstance(arg, ir.Held))
        uses = tuple(arg.value for arg in what.sources if isinstance(arg, ir.Held))
        widths = tuple((arg.value, arg.width) for arg in (*what.dests, *what.sources) if isinstance(arg, ir.Held))
        return lir.Insn(at, (at, at), what, defines, uses, widths=widths)

    blocks = []
    for block in body.blocks:
        insns = []
        if block.at == body.entry:
            at = block.insns[0].at if block.insns else block.at
            insns += [
                insn(ir.Semantics(ir.Operation.BARRIER, "fnstcw", (saved,), ()), at),
                insn(ir.Semantics(ir.Operation.MOVE, "mov", (loaded,), (saved,)), at),
                insn(ir.Semantics(ir.Operation.BINARY, "or", (chopped,), (loaded, ir.Imm(0x0C00, 2))), at),
                insn(ir.Semantics(ir.Operation.MOVE, "mov", (chop,), (chopped,)), at),
            ]
        for one in block.insns:
            if one.what is None or one.what.op is not ir.Operation.FLOAT_STORE or one.what.name != "fisttp":
                insns.append(one)
                continue
            insns += [
                insn(ir.Semantics(ir.Operation.BARRIER, "fldcw", (), (chop,)), one.at),
                replace(one, what=replace(one.what, name="fistp")),
                insn(ir.Semantics(ir.Operation.BARRIER, "fldcw", (), (saved,)), one.at),
            ]
        blocks.append(replace(block, insns=tuple(insns)))
    return replace(body, blocks=tuple(blocks))


class FloatAlloc(LIRTransform):
    name = "floatalloc"

    def __init__(self, frame=None, *, basic_semantics: bool = True, cpu: str | targets.Profile = "386"):
        self.frame = frame
        self.basic_semantics = basic_semantics
        self.cpu = targets.profile(cpu)

    def transform(self, body):
        return allocated(body, self.frame, basic_semantics=self.basic_semantics, cpu=self.cpu)
