"""Simplifications that depend on the final physical register assignment."""

from dataclasses import replace

from iced_x86 import Decoder
from iced_x86 import Mnemonic
from iced_x86 import OpAccess
from iced_x86 import Register
from iced_x86 import Register_
from iced_x86 import RflagsBits
from iced_x86 import FlowControl
from iced_x86 import RegisterExt

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import target
from qbopt.model.passes import LIRTransform


class Peephole(LIRTransform):
    name = "peephole"

    def __init__(self, frame=None):
        self.frame = frame

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        from qbopt.backend import phielim
        from qbopt.backend import copyprop
        from qbopt.backend import copysink
        from qbopt.backend import regthrash
        from qbopt.backend import spillforward
        from qbopt.backend import storecombine

        # Before the rest: a thrashed copy is one fewer instruction for
        # everything below to reason about, and it is the only pass here
        # that can remove a copy the coalescer refused on colourability.
        body = regthrash.thrashed(phielim.unsplit(body))
        body = copyprop.forwarded(body)
        body = copysink.sunk(body)
        body = spillforward.forwarded(body)
        body = storecombine.combined(body)
        body = pushed_constants(body)
        return self._frame(waits(zero_compares(tested(zeroes(addresses(overwritten(shuttles(commuted(constants(pushes(body)))))))))))

    def _frame(self, body):
        """Drop only synthetic reservations when no added stack storage remains."""
        from qbopt.objectfile.module import Space

        if self.frame is None or not self.frame.size:
            return body
        for one in body.insns:
            if one.what is None or one.what.op is ir.Operation.BARRIER:
                return body
            for arg in (*one.what.sources, *one.what.dests):
                if isinstance(arg, (ir.Mem, ir.Address, ir.Imm)):
                    address = arg.address if isinstance(arg, ir.Imm) else arg.addr
                    if address is None and not isinstance(arg, ir.Imm):
                        return body
                    if (
                        isinstance(arg, (ir.Mem, ir.Address))
                        and arg.through in (Register.BP, Register.EBP, Register.SP, Register.ESP)
                        and (address is None or address.space is not Space.FRAME)
                    ):
                        return body
                    if address is not None and address.space is Space.FRAME and address.disp < self.frame.floor:
                        return body
        return replace(
            body,
            blocks=tuple(
                replace(block, insns=tuple(lir.without(block.insns, lambda one: one.frame_adjust)))
                for block in body.blocks
            ),
        )


def pushed_constants(body: lir.LirBody) -> lir.LirBody:
    """Materialize a call's literal once when both stack and register need it."""
    blocks = []
    for block in body.blocks:
        out = []
        for one in block.insns:
            if out:
                push = out[-1]
                match push.what, one.what:
                    case (
                        ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Imm() as literal,)),
                        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg() as dest,), (source,)),
                    ):
                        if (
                            literal == source
                            and literal.address is None
                            and dest.width == literal.width in (2, 4)
                            and target.WIDTHS.get(dest.register) == dest.width
                            and RegisterExt.full_register32(dest.register) not in (Register.ESP, Register.EBP)
                            and one.covers == (one.at, one.at)
                            and push.covers is not None
                            and push.covers[1] == one.at
                            and not push.defines
                            and not push.uses
                            and not one.uses
                            and all(
                                not (
                                    item.clobbers
                                    or item.requires
                                    or item.delivers
                                    or item.spread
                                    or item.group is not None
                                    or item.symbol is True
                                    or item.frame_adjust
                                    or item.spill_reload
                                )
                                for item in (push, one)
                            )
                        ):
                            out[-1] = replace(one, at=push.at, covers=(push.at, push.at), symbol=False)
                            out.append(replace(push, what=replace(push.what, sources=(dest,)), uses=one.defines))
                            continue
            out.append(one)
        blocks.append(replace(block, insns=tuple(out)))
    return replace(body, blocks=tuple(blocks))


def pushes(body: lir.LirBody) -> lir.LirBody:
    """Two adjacent immediate word pushes have one dword's stack layout."""
    blocks = []
    for block in body.blocks:
        out = []
        index = 0
        while index < len(block.insns):
            pair = block.insns[index : index + 2]
            if len(pair) == 2 and all(
                not (
                    one.clobbers
                    or one.requires
                    or one.delivers
                    or one.defines
                    or one.uses
                    or one.symbol is True
                    or one.spread
                )
                for one in pair
            ):
                match pair[0].what, pair[1].what:
                    case (
                        ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Imm(high, 2, None),)),
                        ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Imm(low, 2, None),)),
                    ):
                        what = ir.Semantics(
                            ir.Operation.PUSH, "push", (), (ir.Imm(((high & 0xFFFF) << 16) | (low & 0xFFFF), 4),)
                        )
                        combined = replace(pair[0], what=what)
                        folded = lir.without((combined, pair[1]), lambda one: one is pair[1])
                        if len(folded) == 1:
                            out.extend(folded)
                            index += 2
                            continue
            out.append(block.insns[index])
            index += 1
        blocks.append(replace(block, insns=tuple(out)))
    return replace(body, blocks=tuple(blocks))


def _lanes(register):
    if register in target.SEGMENTS:
        return {(register, byte) for byte in range(target.width_of(register))}
    full = RegisterExt.full_register32(register)
    if full not in {Register.EAX, Register.EBX, Register.ECX, Register.EDX, Register.ESI, Register.EDI, Register.EBP}:
        return set()
    start = int(register in {Register.AH, Register.BH, Register.CH, Register.DH})
    return {(full, byte) for byte in range(start, start + RegisterExt.size(register))}


def commuted(body: lir.LirBody) -> lir.LirBody:
    """Use a saved accumulator in place for commutative two-address operations."""
    blocks = []
    for block in body.blocks:
        insns = list(block.insns)
        removed = set()
        for index in range(2, len(insns)):
            saved, copied, combined = insns[index - 2 : index + 1]
            if any(
                id(one) in removed
                or one.clobbers
                or one.requires
                or one.delivers
                or one.spread
                or one.group is not None
                for one in (saved, copied, combined)
            ):
                continue
            if copied.symbol is True or combined.symbol is True:
                continue
            match saved.what, copied.what, combined.what:
                case (
                    ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg() as temporary,), (ir.Reg() as accumulator,)),
                    ir.Semantics(ir.Operation.MOVE, "mov", (destination,), (ir.Reg() as term,)),
                    ir.Semantics(ir.Operation.BINARY, name, (result,), (left, right)),
                ):
                    if (
                        name not in {"add", "and", "or", "xor"}
                        or not accumulator == destination == result == left
                        or right != temporary
                        or not accumulator.width == temporary.width == term.width
                        or accumulator.width not in (2, 4)
                        or _lanes(accumulator.register) & _lanes(temporary.register)
                    ):
                        continue
                    insns[index] = replace(combined, what=replace(combined.what, sources=(accumulator, term)))
                    removed.add(id(copied))
        blocks.append(replace(block, insns=tuple(lir.without(insns, lambda one: id(one) in removed))))
    return replace(body, blocks=tuple(blocks))


def shuttles(body: lir.LirBody) -> lir.LirBody:
    """Do tied work in its source register when a copy restores the result.

    After allocation, ``T = S; T = op(T); S = T`` leaves both registers
    holding the result. ``S = op(S); T = S`` leaves exactly the same physical
    state and flags, while removing one move. This is deliberately after
    allocation: globally joining the two virtual intervals can make a
    colourable graph spill, whereas this local rewrite changes no interval.
    """
    blocks = []
    for block in body.blocks:
        out = []
        index = 0
        while index < len(block.insns):
            triple = block.insns[index : index + 3]
            changed = _shuttle(triple) if len(triple) == 3 else None
            if changed is not None:
                out.extend(changed)
                index += 3
                continue
            out.append(block.insns[index])
            index += 1
        blocks.append(replace(block, insns=tuple(out)))
    return replace(body, blocks=tuple(blocks))


def _shuttle(parts: tuple[lir.Insn, ...]) -> tuple[lir.Insn, lir.Insn] | None:
    saved, combined, restored = parts
    if any(
        one.what is None
        or one.clobbers
        or one.requires
        or one.delivers
        or one.spread
        or one.group is not None
        or one.symbol is True
        or one.frame_adjust
        or one.spill_reload
        for one in parts
    ):
        return None
    if any(one.covers is None or one.covers[0] != one.covers[1] for one in (saved, restored)):
        return None
    match saved.what, combined.what, restored.what:
        case (
            ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg() as temporary,), (ir.Reg() as source,)),
            ir.Semantics(operation, name, (destination,), operands, target_),
            ir.Semantics(ir.Operation.MOVE, "mov", (last_destination,), (last_source,)),
        ):
            if (
                operation not in {ir.Operation.BINARY, ir.Operation.UNARY, ir.Operation.MULTIPLY}
                or destination != temporary
                or not operands
                or operands[0] != temporary
                or last_destination != source
                or last_source != temporary
                or temporary.width != source.width
                or temporary.width not in {1, 2, 4}
                or ir.root(temporary.register) == ir.root(source.register)
            ):
                return None
        case _:
            return None

    rewritten = ir.Semantics(
        operation,
        name,
        tuple(_register_operand(one, temporary.register, source.register) for one in combined.what.dests),
        tuple(_register_operand(one, temporary.register, source.register) for one in combined.what.sources),
        target_,
    )
    from qbopt.backend import select

    if select.emit(rewritten) is None:
        return None
    reverse = ir.Semantics(ir.Operation.MOVE, "mov", (temporary,), (source,))
    return replace(combined, what=rewritten), replace(restored, what=reverse)


def _register_operand(one: ir.Loc, before: Register_, after: Register_) -> ir.Loc:
    """One allocated operand with aliases of `before` renamed to `after`."""
    if isinstance(one, ir.Reg) and ir.root(one.register) == ir.root(before):
        return replace(one, register=target.named(after, one.width))
    if isinstance(one, ir.Mem) and ir.root(before) in {ir.root(one.through), ir.root(one.index_through)}:
        return replace(
            one,
            through=target.named(after, target.width_of(one.through))
            if ir.root(one.through) == ir.root(before)
            else one.through,
            index_through=target.named(after, target.width_of(one.index_through))
            if ir.root(one.index_through) == ir.root(before)
            else one.index_through,
        )
    if isinstance(one, ir.Address):
        through = (
            target.named(after, target.width_of(one.through))
            if ir.root(one.through) == ir.root(before)
            else one.through
        )
        index = target.named(after, target.width_of(one.index)) if ir.root(one.index) == ir.root(before) else one.index
        return replace(one, through=through, index=index)
    return one


def _register_effects(one, *, may_write=False, flags: bool = False):
    from qbopt.backend import select
    from qbopt.frontend.declen import INFO
    from qbopt.frontend.declen import READS

    if one.clobbers or one.symbol is True:
        return None
    if one.what is None or one.what.op is ir.Operation.BARRIER:
        node = getattr(one.op, "node", None)
        decoded = getattr(node, "insn", None)
        if not isinstance(node, ir.Opaque) or decoded is None:
            return None
        instructions = (decoded.insn,)
    else:
        if one.what.op is ir.Operation.NOTHING and not one.what.name:
            return set(), set()
        encoded = select.emit(one.what)
        if encoded is None:
            return None
        instructions = tuple(Decoder(16, encoded.code))
    reads = {lane for _, register in one.requires for lane in _lanes(register)}
    writes = {lane for _, register in one.delivers for lane in _lanes(register)}
    for insn in instructions:
        if insn.is_invalid or insn.flow_control != FlowControl.NEXT:
            return None
        if flags:
            reads.update(_flag_lanes(insn.rflags_read) - writes)
            if insn.mnemonic in {
                Mnemonic.ADD,
                Mnemonic.SUB,
                Mnemonic.AND,
                Mnemonic.OR,
                Mnemonic.XOR,
                Mnemonic.CMP,
                Mnemonic.TEST,
                Mnemonic.INC,
                Mnemonic.DEC,
                Mnemonic.NEG,
                Mnemonic.ADC,
                Mnemonic.SBB,
                Mnemonic.SAHF,
                Mnemonic.CLC,
                Mnemonic.STC,
                Mnemonic.CMC,
            }:
                writes.update(_flag_lanes(insn.rflags_written | insn.rflags_cleared | insn.rflags_set))
        for access in INFO.info(insn).used_registers():
            lanes = _lanes(access.register)
            if access.access in READS:
                reads.update(lanes - writes)
        for access in INFO.info(insn).used_registers():
            if (
                access.access in (OpAccess.WRITE, OpAccess.READ_WRITE)
                or may_write
                and access.access in (OpAccess.COND_WRITE, OpAccess.READ_COND_WRITE)
            ):
                writes.update(_lanes(access.register))
    return reads, writes


def _flag_lanes(mask: int) -> set[tuple[int, int]]:
    return {(Register.NONE, bit) for bit in range(32) if mask & (1 << bit)}


def _frame_cell(cell: ir.Mem) -> bool:
    """A bp-relative slot whose bytes the displacement alone names."""
    from qbopt.objectfile.module import Space

    return (
        cell.addr is not None and cell.addr.space is Space.FRAME and cell.through == Register.BP and cell.base is None
    )


def _overlapping(a: ir.Mem, b: ir.Mem) -> bool:
    """Whether two such slots share a byte -- arithmetic on the displacements."""
    if not _frame_cell(a) or not _frame_cell(b):
        return True
    return a.addr.disp < b.addr.disp + b.width and b.addr.disp < a.addr.disp + a.width


def _frame_written(one: lir.Insn) -> "ir.Mem | None":
    """The one frame slot this instruction writes, if that is all it writes.

    The allocator's own store says so itself. Any other instruction has to
    prove it from the operation it came from, because an inserted store
    carries the `op` of whatever it stands beside and that one's stores are
    not its own.
    """
    from qbopt.objectfile.module import Space

    what = one.what
    if what is None:
        return None
    cells = [dest for dest in what.dests if isinstance(dest, ir.Mem)]
    if len(cells) != 1 or not _frame_cell(cells[0]):
        return None
    stores = getattr(one.op, "stores", ())
    if not one.spill_store and (
        len(stores) > 1
        or any(ref.addr is None or ref.addr.space is not Space.FRAME or ref.base is not None for ref in stores)
    ):
        return None
    return cells[0]


def overwritten(body: lir.LirBody) -> lir.LirBody:
    """Remove overwritten moves, pure arithmetic and owned spill reloads.

    What is dead on exit from the block is a fact about the whole body, not
    something a backward walk of one block can assume away: a phi's parallel
    copy is written as the last instruction there, which is exactly where
    "assume everything live" refuses to look.
    """
    from qbopt.backend import liveness

    exits = liveness.dead_at_exit(body)
    blocks = []
    for block in body.blocks:
        dead, redundant = set(exits[block.at]), set()
        for one in reversed(block.insns):
            if liveness._terminator(one.what):
                if one.what.op is ir.Operation.BRANCH:
                    dead -= _flag_lanes(0xFFFFFFFF)
                continue
            effects = _register_effects(one, flags=True)
            if effects is None:
                dead.clear()
                continue
            match one.what:
                case ir.Semantics(
                    ir.Operation.MOVE, "mov", (ir.Reg() as dest,), (ir.Reg() | ir.Imm() | ir.Mem() as source,)
                ):
                    lanes = _lanes(dest.register)
                    if (
                        lanes
                        and lanes <= dead
                        and dest.width == source.width
                        and not one.requires
                        and not one.delivers
                        and (
                            isinstance(source, ir.Reg)
                            or isinstance(source, ir.Imm)
                            and source.address is None
                            or isinstance(source, ir.Mem)
                            and one.spill_reload
                        )
                    ):
                        redundant.add(id(one))
                        continue
                case ir.Semantics(ir.Operation.EXTEND, "movsx" | "movzx", (ir.Reg() as dest,), (ir.Reg(),)):
                    # Pure, and nothing else it writes. Lowering a divide's
                    # sign word as `cwd` leaves the widening it replaced
                    # behind, with a register and an instruction to its name.
                    if (
                        effects[1]
                        and effects[1] <= dead
                        and _lanes(dest.register)
                        and not one.requires
                        and not one.delivers
                        and not one.spread
                        and one.group is None
                        and one.symbol is not True
                    ):
                        redundant.add(id(one))
                        continue
                case ir.Semantics(
                    ir.Operation.BINARY, "add" | "sub" | "and" | "or" | "xor", (ir.Reg() as dest,), sources
                ):
                    if (
                        effects[1]
                        and effects[1] <= dead
                        and _lanes(dest.register)
                        and not one.requires
                        and not one.delivers
                        and not one.spread
                        and one.group is None
                        and all(
                            isinstance(source, ir.Reg) or isinstance(source, ir.Imm) and source.address is None
                            for source in sources
                        )
                    ):
                        redundant.add(id(one))
                        continue
            reads, writes = effects
            dead = (dead | writes) - reads
        blocks.append(replace(block, insns=tuple(lir.without(block.insns, lambda one: id(one) in redundant))))
    return replace(body, blocks=tuple(blocks))


def _scaled_address(parts: tuple[lir.Insn, ...], *, flags_dead: bool = False) -> lir.Insn | None:
    if len(parts) not in (3, 4) or any(one.what is None or one.clobbers or one.symbol is True for one in parts):
        return None
    copy, shift, add = parts[:3]
    if any(one.spread or (one.covers and one.covers[0] != one.covers[1]) for one in (shift, add)):
        return None
    match copy.what:
        case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg() as dest,), (ir.Reg() as source,)):
            pass
        case _:
            return None
    if (
        dest.width != source.width
        or dest.width not in {2, 4}
        or dest.register not in target.WIDTHS
        or source.register not in target.WIDTHS
        or RegisterExt.full_register32(dest.register) == RegisterExt.full_register32(source.register)
    ):
        return None
    match shift.what, add.what:
        case (
            ir.Semantics(ir.Operation.BINARY, "shl", (shift_dest,), (shift_source, ir.Imm(amount, _, None))),
            ir.Semantics(ir.Operation.BINARY, "add", (add_dest,), (left, right)),
        ):
            if (
                not 1 <= amount <= 3
                or any(one != dest for one in (shift_dest, shift_source, add_dest, left))
                or right != source
            ):
                return None
        case _:
            return None
    if not flags_dead:
        if len(parts) != 4:
            return None
        match parts[3].what:
            case ir.Semantics(ir.Operation.BINARY, "shl", (last_dest,), (last_source, ir.Imm(count, _, None))):
                if last_dest != dest or last_source != dest or not 0 < count < dest.width * 8:
                    return None
            case _:
                return None
    base = RegisterExt.full_register32(source.register)
    if base == Register.ESP:
        return None
    # For a word result only the low sixteen address bits are used. Unknown
    # upper source bits cannot affect them; LEA performs no memory access.
    what = ir.Semantics(
        ir.Operation.ADDRESS, "lea", (dest,), (ir.Address(None, through=base, index=base, scale=1 << amount),)
    )
    return replace(copy, what=what, defines=add.defines)


def addresses(body: lir.LirBody) -> lir.LirBody:
    """Select LEA for allocated arithmetic when the replaced flags are dead."""
    blocks = []
    for block in body.blocks:
        dead = set()
        flags_dead = False
        for one in reversed(block.insns):
            if flags_dead:
                dead.add(id(one))
            flags_dead = _flags_before(one, flags_dead)
        insns = []
        index = 0
        while index < len(block.insns):
            triple = block.insns[index : index + 3]
            combined = _scaled_address(triple, flags_dead=True) if len(triple) == 3 and id(triple[2]) in dead else None
            if combined is None:
                combined = _scaled_address(block.insns[index : index + 4])
            if combined is not None:
                insns.append(combined)
                index += 3
            else:
                pair = block.insns[index : index + 2]
                combined = _shift_address(pair) if len(pair) == 2 and id(pair[1]) in dead else None
                if combined is not None:
                    folded = (
                        [combined]
                        if pair[0].covers == (pair[1].at, pair[1].at)
                        else lir.without((combined, pair[1]), lambda one: one is pair[1])
                    )
                    if len(folded) == 1:
                        insns.extend(folded)
                        index += 2
                        continue
                insns.append(block.insns[index])
                index += 1
        blocks.append(replace(block, insns=tuple(insns)))
    return replace(body, blocks=tuple(blocks))


def _shift_address(parts: tuple[lir.Insn, ...]) -> lir.Insn | None:
    copy, shift = parts
    if any(one.what is None or one.clobbers or one.symbol is True or one.spread for one in parts):
        return None
    match copy.what, shift.what:
        case (
            ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg() as dest,), (ir.Reg() as source,)),
            ir.Semantics(ir.Operation.BINARY, "shl", (written,), (read, ir.Imm(1, _, None))),
        ):
            if (
                written != dest
                or read != dest
                or dest.width != source.width
                or dest.width not in {2, 4}
                or dest.register not in target.WIDTHS
                or source.register not in target.WIDTHS
            ):
                return None
        case _:
            return None
    base = RegisterExt.full_register32(source.register)
    if base == Register.ESP or base == RegisterExt.full_register32(dest.register):
        return None
    what = ir.Semantics(ir.Operation.ADDRESS, "lea", (dest,), (ir.Address(None, through=base, index=base),))
    if copy.covers == (shift.at, shift.at):
        return replace(shift, what=what, uses=copy.uses)
    return replace(copy, what=what, defines=shift.defines)


def _flags_before(one: lir.Insn, flags_dead: bool) -> bool:
    if one.what is None or one.clobbers:
        return False
    match one.what:
        case ir.Semantics(ir.Operation.COMPARE, "cmp" | "test"):
            return True
        case ir.Semantics(ir.Operation.BINARY, "add" | "sub" | "and" | "or" | "xor"):
            return True
        case ir.Semantics(ir.Operation.UNARY, "neg"):
            return True
        case ir.Semantics(ir.Operation.MOVE, "mov") | ir.Semantics(ir.Operation.ADDRESS, "lea"):
            return flags_dead
        case ir.Semantics(ir.Operation.EXTEND, "movsx" | "movzx" | "cwd" | "cdq"):
            return flags_dead
        case ir.Semantics(ir.Operation.PUSH, "push") | ir.Semantics(ir.Operation.POP, "pop"):
            return flags_dead
        case ir.Semantics(ir.Operation.NOTHING, None | "") | ir.Semantics(ir.Operation.JUMP):
            return flags_dead
    return False


_ZERO_BRANCHES = {"je": RflagsBits.ZF, "jne": RflagsBits.ZF, "js": RflagsBits.SF, "jns": RflagsBits.SF}
_BRANCH_FLAGS = {
    **_ZERO_BRANCHES,
    "jl": RflagsBits.SF | RflagsBits.OF,
    "jge": RflagsBits.SF | RflagsBits.OF,
    "jle": RflagsBits.ZF | RflagsBits.SF | RflagsBits.OF,
    "jg": RflagsBits.ZF | RflagsBits.SF | RflagsBits.OF,
    "jb": RflagsBits.CF,
    "jae": RflagsBits.CF,
    "jbe": RflagsBits.CF | RflagsBits.ZF,
    "ja": RflagsBits.CF | RflagsBits.ZF,
}
_ARITHMETIC = RflagsBits.OF | RflagsBits.SF | RflagsBits.ZF | RflagsBits.AF | RflagsBits.CF | RflagsBits.PF
# What `cmp r,0` leaves that the instruction computing r may not: inc keeps
# the carry, add and subtract set carry and overflow from their operands.
_DIFFERING = _flag_lanes(RflagsBits.OF | RflagsBits.CF | RflagsBits.AF)


def tested(body: lir.LirBody) -> lir.LirBody:
    """`inc edi; cmp edi,0; jne` is `inc edi; jne`.

    An add, subtract, logic or unary operation sets ZF and SF from its result
    exactly as a zero test of that result does. The test goes where only its
    branch reads those two, and no later instruction reads the carry,
    overflow or adjust flags it would have cleared.
    """
    live = _flags_live_out(body)
    blocks = []
    for block in body.blocks:
        insns = list(block.insns)
        # Moves change no flag, so the three may have a phi's copies between them.
        work = [index for index, one in enumerate(insns) if not _nothing(one)]
        test_at = len(work) - 2
        while test_at >= 1 and _moves(insns[work[test_at]]):
            test_at -= 1
        register = _zero_tested(insns[work[test_at]]) if test_at >= 1 else None
        before_at = test_at - 1
        while register is not None and before_at >= 0 and _moves(insns[work[before_at]], register):
            before_at -= 1
        if register is not None and before_at >= 0 and work:
            before, test, branch = insns[work[before_at]], insns[work[test_at]], insns[work[-1]]
            if (
                branch.what is not None
                and branch.what.op is ir.Operation.BRANCH
                and branch.what.name in _ZERO_BRANCHES
                and _sets_from(before, register)
                and not live[block.at] & _DIFFERING
            ):
                insns[work[test_at]] = replace(test, what=ir.Semantics(ir.Operation.NOTHING, ""), defines=(), uses=(), widths=())
        blocks.append(replace(block, insns=tuple(insns)))
    return replace(body, blocks=tuple(blocks))


_ADJUST = _flag_lanes(RflagsBits.AF)


def zero_compares(body: lir.LirBody) -> lir.LirBody:
    """`cmp r,0; jcc` is `or r,r; jcc`, a byte shorter.

    Both clear carry and overflow and set zero, sign and parity from r; only
    the adjust flag differs, so the branch must be the next work and nothing
    after the block may read AF. The branch reading straight after keeps OF
    clear of anything between -- DOSBox's dynamic core loses it across OR and
    SAHF.
    """
    live = _flags_live_out(body)
    blocks = []
    for block in body.blocks:
        insns = list(block.insns)
        work = [index for index, one in enumerate(insns) if not _nothing(one)]
        at = len(work) - 2
        # Moves change no flag, so a phi's copies may stand between the two.
        while at >= 0 and _moves(insns[work[at]]):
            at -= 1
        if at >= 0 and not live[block.at] & _ADJUST:
            test, branch = insns[work[at]], insns[work[-1]]
            register = _zero_tested(test)
            if (
                register is not None
                and register.register in target.WIDTHS
                and branch.what is not None
                and branch.what.op is ir.Operation.BRANCH
                and branch.what.name in _BRANCH_FLAGS
                and test.what.name == "cmp"
            ):
                insns[work[at]] = replace(test, what=ir.Semantics(ir.Operation.BINARY, "or", (register,), (register, register)))
        blocks.append(replace(block, insns=tuple(insns)))
    return replace(body, blocks=tuple(blocks))


def _moves(one: lir.Insn, register: "ir.Reg | None" = None) -> bool:
    """A plain move, writing nothing that shares a root with `register`."""
    if one.what is None or one.what.op is not ir.Operation.MOVE or one.what.name != "mov" or one.clobbers:
        return False
    return register is None or all(
        not isinstance(dest, ir.Reg) or ir.root(dest.register) != ir.root(register.register) for dest in one.what.dests
    )


def _nothing(one: lir.Insn) -> bool:
    return one.what is not None and one.what.op is ir.Operation.NOTHING and not one.what.name


def _zero_tested(one: lir.Insn) -> "Register_ | None":
    if one.clobbers or one.symbol is True or one.what is None:
        return None
    match one.what:
        case ir.Semantics(ir.Operation.COMPARE, "cmp", _, (ir.Reg() as register, ir.Imm(0, _, None))):
            return register
        case ir.Semantics(ir.Operation.COMPARE, "test", _, (ir.Reg() as register, ir.Reg() as other)) if other == register:
            return register
    return None


def _sets_from(one: lir.Insn, register: ir.Reg) -> bool:
    if one.clobbers or one.what is None:
        return False
    match one.what:
        case ir.Semantics(ir.Operation.BINARY, "add" | "sub" | "and" | "or" | "xor", (ir.Reg() as dest,), _):
            return dest == register
        case ir.Semantics(ir.Operation.UNARY, "inc" | "dec" | "neg", (ir.Reg() as dest,), _):
            return dest == register
    return False


def _flags_live_out(body: lir.LirBody) -> dict[int, set]:
    """Which flag lanes something may read after each block's last instruction."""
    every = _flag_lanes(_ARITHMETIC)
    # No calling convention passes the adjust flag in or out: a callee, a
    # caller after a return, and whatever runs after the body leaves may read
    # any other flag, but only an instruction here that reads AF reads it.
    exits = every - _ADJUST

    def effects(one: lir.Insn) -> tuple[set, set]:
        if one.what is not None and one.what.op is ir.Operation.BRANCH:
            return _flag_lanes(_BRANCH_FLAGS.get(one.what.name, _ARITHMETIC)), set()
        if one.what is not None and one.what.op is ir.Operation.JUMP or _nothing(one):
            return set(), set()
        if one.what is not None and one.what.op in (ir.Operation.CALL, ir.Operation.RETURN):
            return set(exits), set()
        found = _register_effects(one, flags=True)
        if found is None:
            return every, set()
        reads, writes = found
        return {lane for lane in reads if lane[0] == Register.NONE}, {lane for lane in writes if lane[0] == Register.NONE}

    steps = {block.at: [effects(one) for one in block.insns] for block in body.blocks}
    live_in = {block.at: set() for block in body.blocks}
    out = {}
    changed = True
    while changed:
        changed = False
        for block in reversed(body.blocks):
            after = set().union(*(live_in.get(at, exits) for at in block.succ)) if block.succ else set(exits)
            out[block.at] = after
            live = set(after)
            for reads, writes in reversed(steps[block.at]):
                live = (live - writes) | reads
            if live != live_in[block.at]:
                live_in[block.at] = live
                changed = True
    return out


def zeroes(body: lir.LirBody) -> lir.LirBody:
    """Use XOR for zero only when later integer work replaces every arithmetic flag."""
    live = _flags_live_out(body)
    blocks = []
    for block in body.blocks:
        # What the block's successors read, not a guess: `mov bx,0; jmp` to a
        # block that sets its own flags first zeroes with xor too.
        flags_dead = not live[block.at]
        insns = []
        for one in reversed(block.insns):
            what = one.what
            previous = _flags_before(one, flags_dead)
            if what is not None and not one.clobbers:
                match what:
                    case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg() as dest,), (ir.Imm(0, width, None),)):
                        if (
                            flags_dead
                            and dest.width == width
                            and width in {2, 4}
                            and dest.register in target.WIDTHS
                            and one.symbol is not True
                        ):
                            one = replace(one, what=ir.Semantics(ir.Operation.BINARY, "xor", (dest,), (dest, dest)))
            flags_dead = previous
            insns.append(one)
        blocks.append(replace(block, insns=tuple(reversed(insns))))
    return replace(body, blocks=tuple(blocks))


_WAITING = frozenset(
    {
        "wait",
        "fwait",
        "fld",
        "fild",
        "fst",
        "fstp",
        "fist",
        "fistp",
        "fadd",
        "faddp",
        "fiadd",
        "fsub",
        "fsubp",
        "fsubr",
        "fsubrp",
        "fisub",
        "fisubr",
        "fmul",
        "fmulp",
        "fimul",
        "fdiv",
        "fdivp",
        "fdivr",
        "fdivrp",
        "fidiv",
        "fidivr",
        "fchs",
        "fabs",
        "fsqrt",
        "fxch",
        "fcom",
        "fcomp",
        "fcompp",
        "fucom",
        "fucomp",
        "fucompp",
    }
)


def waits(body: lir.LirBody) -> lir.LirBody:
    """An immediately following waiting instruction already checks pending FP exceptions.

    Intel SDM Vol. 1 section 8.3.12. Never cross integer work, an unknown
    instruction, a non-waiting control instruction, or a block boundary.
    """
    blocks = []
    for block in body.blocks:
        following = None
        redundant = set()
        for one in reversed(block.insns):
            what = one.what
            if what is not None and what.op is ir.Operation.NOTHING and not what.name:
                continue
            if (
                what is not None
                and what.name in {"wait", "fwait"}
                and following is not None
                and following.name in _WAITING
            ):
                redundant.add(id(one))
            else:
                following = what
        blocks.append(replace(block, insns=tuple(lir.without(block.insns, lambda one: id(one) in redundant))))
    return replace(body, blocks=tuple(blocks))


def constants(body: lir.LirBody) -> lir.LirBody:
    """Reuse identical scalar register contents within a straight-line move sequence."""
    blocks = []
    for block in body.blocks:
        held = {}
        redundant = set()
        for one in block.insns:
            what = one.what
            if (
                what is not None
                and what.op is ir.Operation.NOTHING
                and what.name in (None, "")
                and not what.dests
                and not what.sources
                and what.target is None
                and not one.clobbers
                and not one.defines
            ):
                continue
            move = what is not None and what.op is ir.Operation.MOVE and what.name == "mov"
            extend = (
                what is not None
                and what.op is ir.Operation.EXTEND
                and what.name in {"cwd", "cdq", "movsx"}
                and len(what.dests) == len(what.sources) == 1
                and all(isinstance(arg, ir.Reg) for arg in (*what.dests, *what.sources))
            )
            if not move and not extend:
                held.clear()
                continue
            candidate = None
            if move and len(what.dests) == len(what.sources) == 1:
                dest, source = what.dests[0], what.sources[0]
                if (
                    isinstance(dest, ir.Reg)
                    and dest.register in target.WIDTHS
                    and isinstance(source, (ir.Reg, ir.Imm))
                    and dest.width == source.width
                ):
                    if isinstance(source, ir.Imm) and source.address is None:
                        candidate = dest, source.value & ((1 << (dest.width * 8)) - 1)
                    elif isinstance(source, ir.Reg) and source.register in target.WIDTHS:
                        candidate = dest, held.setdefault(source, object())
            if candidate is not None and not one.clobbers and held.get(candidate[0]) == candidate[1]:
                redundant.add(id(one))
                continue
            written = {RegisterExt.full_register32(reg) for reg in one.clobbers}
            written.update(
                RegisterExt.full_register32(dest.register) for dest in what.dests if isinstance(dest, ir.Reg)
            )
            held = {
                dest: value for dest, value in held.items() if RegisterExt.full_register32(dest.register) not in written
            }
            if candidate is not None and not one.clobbers:
                held[candidate[0]] = candidate[1]
        blocks.append(replace(block, insns=tuple(lir.without(block.insns, lambda one: id(one) in redundant))))
    return replace(body, blocks=tuple(blocks))
