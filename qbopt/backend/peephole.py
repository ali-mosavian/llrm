"""Simplifications that depend on the final physical register assignment."""

from dataclasses import replace

from iced_x86 import Decoder, FlowControl, OpAccess, Register, RegisterExt

from qbopt.model import ir, lir
from qbopt.backend import target
from qbopt.model.passes import LIRTransform


class Peephole(LIRTransform):
    name = "peephole"

    def __init__(self, frame=None):
        self.frame = frame

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        from qbopt.backend import copyprop, spillforward, storecombine
        body = copyprop.forwarded(body)
        body = spillforward.forwarded(body)
        body = reloads(body)
        body = storecombine.combined(body)
        return self._frame(waits(zeroes(addresses(overwritten(commuted(constants(pushes(body))))))))

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
                    if (isinstance(arg, (ir.Mem, ir.Address))
                        and arg.through in (Register.BP, Register.EBP, Register.SP, Register.ESP)
                        and (address is None or address.space is not Space.FRAME)):
                        return body
                    if address is not None and address.space is Space.FRAME and address.disp < self.frame.floor:
                        return body
        return replace(body, blocks=tuple(replace(block, insns=tuple(
            lir.without(block.insns, lambda one: one.frame_adjust))) for block in body.blocks))


def pushes(body: lir.LirBody) -> lir.LirBody:
    """Two adjacent immediate word pushes have one dword's stack layout."""
    blocks = []
    for block in body.blocks:
        out = []
        index = 0
        while index < len(block.insns):
            pair = block.insns[index:index + 2]
            if len(pair) == 2 and all(not (one.clobbers or one.requires or one.delivers or one.defines
                                           or one.uses or one.symbol is True or one.spread) for one in pair):
                match pair[0].what, pair[1].what:
                    case (ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Imm(high, 2, None),)),
                          ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Imm(low, 2, None),))):
                        what = ir.Semantics(ir.Operation.PUSH, "push", (),
                                            (ir.Imm(((high & 0xffff) << 16) | (low & 0xffff), 4),))
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
            saved, copied, combined = insns[index - 2:index + 1]
            if any(id(one) in removed or one.clobbers or one.requires or one.delivers
                   or one.spread or one.group is not None
                   for one in (saved, copied, combined)):
                continue
            if copied.symbol is True or combined.symbol is True:
                continue
            match saved.what, copied.what, combined.what:
                case (ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg() as temporary,), (ir.Reg() as accumulator,)),
                      ir.Semantics(ir.Operation.MOVE, "mov", (destination,), (ir.Reg() as term,)),
                      ir.Semantics(ir.Operation.BINARY, name, (result,), (left, right))):
                    if (name not in {"add", "and", "or", "xor"}
                        or not accumulator == destination == result == left or right != temporary
                        or not accumulator.width == temporary.width == term.width
                        or accumulator.width not in (2, 4)
                        or _lanes(accumulator.register) & _lanes(temporary.register)):
                        continue
                    insns[index] = replace(combined, what=replace(combined.what, sources=(accumulator, term)))
                    removed.add(id(copied))
        blocks.append(replace(block, insns=tuple(lir.without(insns, lambda one: id(one) in removed))))
    return replace(body, blocks=tuple(blocks))


def _register_effects(one, *, may_write=False):
    from qbopt.backend import select
    from qbopt.frontend.declen import INFO, READS

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
    writes = set()
    for insn in instructions:
        if insn.is_invalid or insn.flow_control != FlowControl.NEXT:
            return None
        for access in INFO.info(insn).used_registers():
            lanes = _lanes(access.register)
            if access.access in READS:
                reads.update(lanes - writes)
        for access in INFO.info(insn).used_registers():
            if (access.access in (OpAccess.WRITE, OpAccess.READ_WRITE)
                or may_write and access.access in (OpAccess.COND_WRITE, OpAccess.READ_COND_WRITE)):
                writes.update(_lanes(access.register))
    return reads, writes


def reloads(body: lir.LirBody) -> lir.LirBody:
    """Reuse allocator-owned frame reloads until either the register or memory changes."""
    from qbopt.objectfile.module import Space

    blocks = []
    for block in body.blocks:
        held, redundant = {}, set()
        for one in block.insns:
            what = one.what
            effects = _register_effects(one)
            if (effects is None or what is None or one.requires or one.delivers
                or what.op not in {ir.Operation.MOVE, ir.Operation.BINARY, ir.Operation.UNARY,
                                   ir.Operation.COMPARE, ir.Operation.EXTEND, ir.Operation.NOTHING}
                or any(not isinstance(dest, ir.Reg) or not _lanes(dest.register) for dest in what.dests)):
                held.clear()
                continue
            writes = effects[1]
            if writes & _lanes(Register.EBP):
                held.clear()
            match what:
                case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg() as register,), (ir.Mem() as cell,)):
                    owned = (one.spill_reload and cell.addr is not None and cell.addr.space is Space.FRAME
                             and cell.through == Register.BP and cell.base is None
                             and register.width == cell.width and not writes & _lanes(Register.EBP))
                    if owned and held.get(register) == cell:
                        redundant.add(id(one))
                        continue
                case _:
                    owned = False
            held = {register: cell for register, cell in held.items()
                    if not _lanes(register.register) & writes}
            if owned:
                held[what.dests[0]] = what.sources[0]
        blocks.append(replace(block, insns=tuple(lir.without(block.insns, lambda one: id(one) in redundant))))
    return replace(body, blocks=tuple(blocks))


def overwritten(body: lir.LirBody) -> lir.LirBody:
    """Remove dead register moves and allocator-owned spill reloads."""
    blocks = []
    for block in body.blocks:
        dead, redundant = set(), set()
        for one in reversed(block.insns):
            effects = _register_effects(one)
            if effects is None:
                dead.clear()
                continue
            match one.what:
                case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg() as dest,), (ir.Reg() | ir.Imm() | ir.Mem() as source,)):
                    lanes = _lanes(dest.register)
                    if (lanes and lanes <= dead and dest.width == source.width
                        and not one.requires and not one.delivers
                        and (isinstance(source, ir.Reg)
                             or isinstance(source, ir.Imm) and source.address is None
                             or isinstance(source, ir.Mem) and one.spill_reload)):
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
    if (dest.width != source.width or dest.width not in {2, 4}
        or dest.register not in target.WIDTHS or source.register not in target.WIDTHS
        or RegisterExt.full_register32(dest.register) == RegisterExt.full_register32(source.register)):
        return None
    match shift.what, add.what:
        case (ir.Semantics(ir.Operation.BINARY, "shl", (shift_dest,), (shift_source, ir.Imm(amount, _, None))),
              ir.Semantics(ir.Operation.BINARY, "add", (add_dest,), (left, right))):
            if (not 1 <= amount <= 3
                or any(one != dest for one in (shift_dest, shift_source, add_dest, left))
                or right != source):
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
    what = ir.Semantics(ir.Operation.ADDRESS, "lea", (dest,),
                        (ir.Address(None, through=base, index=base, scale=1 << amount),))
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
            triple = block.insns[index:index + 3]
            combined = (_scaled_address(triple, flags_dead=True)
                        if len(triple) == 3 and id(triple[2]) in dead else None)
            if combined is None:
                combined = _scaled_address(block.insns[index:index + 4])
            if combined is not None:
                insns.append(combined)
                index += 3
            else:
                pair = block.insns[index:index + 2]
                combined = _shift_address(pair) if len(pair) == 2 and id(pair[1]) in dead else None
                if combined is not None:
                    folded = ([combined] if pair[0].covers == (pair[1].at, pair[1].at)
                              else lir.without((combined, pair[1]), lambda one: one is pair[1]))
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
        case (ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg() as dest,), (ir.Reg() as source,)),
              ir.Semantics(ir.Operation.BINARY, "shl", (written,), (read, ir.Imm(1, _, None)))):
            if (written != dest or read != dest or dest.width != source.width or dest.width not in {2, 4}
                or dest.register not in target.WIDTHS or source.register not in target.WIDTHS):
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
        case ir.Semantics(ir.Operation.NOTHING, None | ""):
            return flags_dead
    return False


def zeroes(body: lir.LirBody) -> lir.LirBody:
    """Use XOR for zero only when later integer work replaces every arithmetic flag."""
    blocks = []
    for block in body.blocks:
        flags_dead = False
        insns = []
        for one in reversed(block.insns):
            what = one.what
            previous = _flags_before(one, flags_dead)
            if what is not None and not one.clobbers:
                match what:
                    case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg() as dest,), (ir.Imm(0, width, None),)):
                        if (flags_dead and dest.width == width and width in {2, 4}
                            and dest.register in target.WIDTHS and one.symbol is not True):
                            one = replace(one, what=ir.Semantics(ir.Operation.BINARY, "xor", (dest,), (dest, dest)))
            flags_dead = previous
            insns.append(one)
        blocks.append(replace(block, insns=tuple(reversed(insns))))
    return replace(body, blocks=tuple(blocks))


_WAITING = frozenset({
    "wait", "fwait", "fld", "fild", "fst", "fstp", "fist", "fistp",
    "fadd", "faddp", "fiadd", "fsub", "fsubp", "fsubr", "fsubrp", "fisub", "fisubr",
    "fmul", "fmulp", "fimul", "fdiv", "fdivp", "fdivr", "fdivrp", "fidiv", "fidivr",
    "fchs", "fabs", "fsqrt", "fxch", "fcom", "fcomp", "fcompp", "fucom", "fucomp", "fucompp",
})


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
            if (what is not None and what.name in {"wait", "fwait"}
                and following is not None and following.name in _WAITING):
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
            if (what is not None and what.op is ir.Operation.NOTHING
                and what.name in (None, "") and not what.dests and not what.sources
                and what.target is None and not one.clobbers and not one.defines):
                continue
            move = what is not None and what.op is ir.Operation.MOVE and what.name == "mov"
            extend = (what is not None and what.op is ir.Operation.EXTEND
                      and what.name in {"cwd", "cdq", "movsx"}
                      and len(what.dests) == len(what.sources) == 1
                      and all(isinstance(arg, ir.Reg) for arg in (*what.dests, *what.sources)))
            if not move and not extend:
                held.clear()
                continue
            candidate = None
            if move and len(what.dests) == len(what.sources) == 1:
                dest, source = what.dests[0], what.sources[0]
                if (isinstance(dest, ir.Reg) and dest.register in target.WIDTHS
                    and isinstance(source, (ir.Reg, ir.Imm)) and dest.width == source.width):
                    if isinstance(source, ir.Imm) and source.address is None:
                        candidate = dest, source.value & ((1 << (dest.width * 8)) - 1)
                    elif isinstance(source, ir.Reg) and source.register in target.WIDTHS:
                        candidate = dest, held.setdefault(source, object())
            if candidate is not None and not one.clobbers and held.get(candidate[0]) == candidate[1]:
                redundant.add(id(one))
                continue
            written = {RegisterExt.full_register32(reg) for reg in one.clobbers}
            written.update(RegisterExt.full_register32(dest.register) for dest in what.dests if isinstance(dest, ir.Reg))
            held = {dest: value for dest, value in held.items()
                    if RegisterExt.full_register32(dest.register) not in written}
            if candidate is not None and not one.clobbers:
                held[candidate[0]] = candidate[1]
        blocks.append(replace(block, insns=tuple(lir.without(block.insns, lambda one: id(one) in redundant))))
    return replace(body, blocks=tuple(blocks))
