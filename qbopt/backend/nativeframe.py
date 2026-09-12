from dataclasses import replace
from dataclasses import dataclass

from iced_x86 import Code
from iced_x86 import Register
from iced_x86 import FlowControl
from iced_x86 import MemorySizeExt

from qbopt.model import ir
from qbopt.model import lir
from qbopt.frontend.declen import INFO
from qbopt.frontend.declen import Insn
from qbopt.frontend.blocks import Block
from qbopt.frontend.declen import WRITES
from qbopt.frontend.stack import touches_sp


@dataclass(frozen=True)
class Entry:
    floor: int
    reserve_at: int
    saved: tuple[int, ...]


@dataclass(frozen=True)
class Plan:
    entry: Entry
    releases: frozenset[int]
    registers: tuple[tuple[int, int], ...] = ()
    outgoing: frozenset[tuple[int, int, int]] = frozenset()
    framed: bool = True
    return_depth: int = 2


def bound(body: lir.LirBody, layout: Plan) -> lir.LirBody:
    def operand(at: int, where: ir.Loc) -> ir.Loc:
        if (
            isinstance(where, ir.Mem)
            and where.addr is not None
            and where.addr.space is ir.Space.FRAME
            and (at, where.addr.disp, where.width) in layout.outgoing
        ):
            return replace(where, stack_argument=True)
        return where

    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                insns=tuple(
                    replace(
                        one,
                        what=replace(
                            one.what,
                            dests=tuple(operand(one.at, where) for where in one.what.dests),
                            sources=tuple(operand(one.at, where) for where in one.what.sources),
                        ),
                    )
                    if one.what is not None
                    else one
                    for one in block.insns
                ),
            )
            for block in body.blocks
        ),
    )


def pins(body: lir.LirBody, layout: Plan) -> dict[int, int]:
    registers = dict(layout.registers)
    result: dict[int, int] = {}
    for one in body.insns:
        if one.at not in registers or one.what is None:
            continue
        for operand in (*one.what.sources, *one.what.dests):
            if isinstance(operand, ir.Held):
                result[operand.value] = registers[one.at]
    return result


def balanced(blocks: tuple[Block, ...], layout: Plan, cleanup: dict[int, int]) -> bool:
    return checked(blocks, layout, cleanup) is not None


def checked(blocks: tuple[Block, ...], layout: Plan, cleanup: dict[int, int]) -> Plan | None:
    by_start = {block.at: block for block in blocks}
    first = next((block for block in blocks if any(insn.at == layout.entry.reserve_at for insn in block.insns)), None)
    if first is None:
        return None
    outgoing: set[tuple[int, int, int]] = set()
    depths = {first.at: layout.entry.floor}
    pending = [first.at]
    while pending:
        block = by_start[pending.pop()]
        depth = depths[block.at]
        active = block is not first
        returned = False
        for insn in block.insns:
            active |= insn.at == layout.entry.reserve_at
            if not active:
                continue
            if insn.at in layout.releases and depth != layout.entry.floor:
                return None
            machine = insn.insn
            if machine.memory_base == Register.BP:
                displacement = machine.memory_displacement & 0xFFFF
                displacement = displacement - 0x10000 if displacement >= 0x8000 else displacement
                if displacement < layout.entry.floor:
                    width = MemorySizeExt.size(machine.memory_size)
                    if (
                        machine.memory_index != Register.NONE
                        or not width
                        or displacement < depth
                        or displacement + width > layout.entry.floor
                    ):
                        return None
                    outgoing.add((insn.at, displacement, width))
            if insn.flow == FlowControl.RETURN:
                if depth != layout.return_depth:
                    return None
                returned = True
                break
            if insn.flow in (FlowControl.CALL, FlowControl.INDIRECT_CALL):
                if insn.at not in cleanup or cleanup[insn.at] < 0:
                    return None
                depth += cleanup[insn.at]
            elif insn.code == Code.LEAVEW:
                depth = 2
            elif (
                insn.code in (Code.ADD_RM16_IMM8, Code.ADD_RM16_IMM16, Code.SUB_RM16_IMM8, Code.SUB_RM16_IMM16)
                and insn.insn.op0_register == Register.SP
            ):
                amount = insn.insn.immediate(1) & 0xFFFF
                amount = amount - 0x10000 if amount >= 0x8000 else amount
                depth += amount if insn.code in (Code.ADD_RM16_IMM8, Code.ADD_RM16_IMM16) else -amount
            elif insn.insn.stack_pointer_increment:
                depth += insn.insn.stack_pointer_increment
            elif touches_sp(insn) or any(
                used.register in (Register.SP, Register.ESP) for used in INFO.info(insn.insn).used_registers()
            ):
                return None
            if insn.flow in (FlowControl.INTERRUPT, FlowControl.INDIRECT_BRANCH, FlowControl.EXCEPTION):
                return None
            if insn.code not in (Code.LEAVEW, Code.POP_R16) and any(
                used.register in (Register.BP, Register.EBP) and used.access in WRITES
                for used in INFO.info(insn.insn).used_registers()
            ):
                return None
        if returned:
            continue
        if not block.succ:
            return None
        for successor in block.succ:
            if successor not in by_start:
                return None
            if successor in depths:
                if depths[successor] != depth or successor == first.at:
                    return None
            else:
                depths[successor] = depth
                pending.append(successor)
    return replace(layout, outgoing=frozenset(outgoing))


def plan(blocks: tuple[Block, ...], start: int) -> Plan | None:
    first = next((block for block in blocks if block.at == start), None)
    if first is None:
        return None
    setup = entry(first.insns)
    if setup is None:
        return frameless(blocks, first)
    releases: set[int] = set()
    registers = [
        (insn.at, insn.insn.op0_register)
        for insn in first.insns
        if insn.at < setup.reserve_at and insn.code == Code.PUSH_R16
    ]
    for block in blocks:
        for index, insn in enumerate(block.insns):
            if insn.flow != FlowControl.RETURN:
                continue
            if index == 0:
                return None
            teardown = block.insns[index - 1]
            if not (
                teardown.code == Code.LEAVEW
                or (
                    setup.floor == -2 * len(setup.saved)
                    and teardown.code == Code.POP_R16
                    and teardown.insn.op0_register == Register.BP
                )
            ):
                return None
            release = teardown.at
            if teardown.code == Code.POP_R16:
                registers.append((teardown.at, Register.BP))
            before = index - 2
            for register in setup.saved:
                while before >= 0 and block.insns[before].code != Code.POP_R16:
                    candidate = block.insns[before]
                    if candidate.flow != FlowControl.NEXT or any(
                        used.register in (Register.SP, Register.ESP, Register.BP, Register.EBP)
                        for used in INFO.info(candidate.insn).used_registers()
                    ):
                        return None
                    before -= 1
                if before < 0 or block.insns[before].insn.op0_register != register:
                    return None
                release = block.insns[before].at
                registers.append((release, register))
                before -= 1
            releases.add(release)
    return Plan(setup, frozenset(releases), tuple(registers)) if releases else None


def frameless(blocks: tuple[Block, ...], first: Block) -> Plan | None:
    """A call-free native leaf that uses neither BP nor an adjustable stack.

    Borland omits a frame from small procedures that need no locals or
    arguments.  Such a body can be rebuilt, but it cannot acquire spill slots:
    BP still belongs to its caller.  `frame.Frame.slot` enforces that second
    half after allocation has made the need observable.
    """
    if not first.insns:
        return None
    saved: list[tuple[int, int]] = []
    entry_pushes: set[int] = set()
    entry_registers: list[tuple[int, int]] = []
    index = 0
    push_widths = {Code.PUSH_R16: 2, Code.PUSH_R32: 4}
    while index < len(first.insns) and first.insns[index].code in push_widths:
        register = first.insns[index].insn.op0_register
        width = push_widths[first.insns[index].code]
        if ir.root(register) not in (Register.EBX, Register.ESI, Register.EDI) or any(
            ir.root(saved_register) == ir.root(register) for saved_register, _ in saved
        ):
            return None
        saved.append((register, width))
        entry_pushes.add(first.insns[index].at)
        entry_registers.append((first.insns[index].at, register))
        index += 1
    if index == len(first.insns):
        return None

    returns: set[int] = set()
    releases: set[int] = set()
    restores: set[int] = set()
    registers = list(entry_registers)
    for block in blocks:
        if not block.succ and block.insns[-1].flow != FlowControl.RETURN:
            return None
        for at, insn in enumerate(block.insns):
            if insn.flow == FlowControl.RETURN:
                returns.add(insn.at)
                before = at - 1
                release = insn.at
                for register, width in saved:
                    pop_code = Code.POP_R32 if width == 4 else Code.POP_R16
                    while before >= 0 and block.insns[before].code != pop_code:
                        candidate = block.insns[before]
                        if candidate.flow != FlowControl.NEXT or touches_sp(candidate):
                            return None
                        before -= 1
                    if before < 0 or block.insns[before].insn.op0_register != register:
                        return None
                    release = block.insns[before].at
                    restores.add(release)
                    registers.append((release, register))
                    before -= 1
                releases.add(release)
    for block in blocks:
        for insn in block.insns:
            if insn.at in entry_pushes | restores | returns:
                continue
            if insn.flow in (FlowControl.CALL, FlowControl.INDIRECT_CALL) or touches_sp(insn):
                return None
            if any(
                used.register in (Register.BP, Register.EBP, Register.SP, Register.ESP)
                for used in INFO.info(insn.insn).used_registers()
            ):
                return None
    if not returns:
        return None
    return Plan(
        Entry(-sum(width for _, width in saved), first.insns[index].at, tuple(register for register, _ in saved)),
        frozenset(releases),
        tuple(registers),
        framed=False,
        return_depth=0,
    )


def entry(insns: tuple[Insn, ...]) -> Entry | None:
    if len(insns) < 3:
        return None
    push, establish = insns[:2]
    if (
        push.code != Code.PUSH_R16
        or push.insn.op0_register != Register.BP
        or establish.code != Code.MOV_R16_RM16
        or establish.insn.op0_register != Register.BP
        or establish.insn.op1_register != Register.SP
        or push.end != establish.at
    ):
        return None
    index, floor = 2, 0
    allocation = insns[index]
    if allocation.code in (Code.SUB_RM16_IMM8, Code.SUB_RM16_IMM16) and allocation.insn.op0_register == Register.SP:
        size = allocation.insn.immediate(1)
        if size > 0x7FFF or size % 2:
            return None
        floor -= size
        index += 1
    saved: list[int] = []
    while index < len(insns) and insns[index].code == Code.PUSH_R16:
        register = insns[index].insn.op0_register
        if register not in (Register.BX, Register.SI, Register.DI) or register in saved:
            return None
        saved.append(register)
        floor -= 2
        index += 1
    if index == len(insns) or any(a.end != b.at for a, b in zip(insns[:index], insns[1 : index + 1], strict=True)):
        return None
    return Entry(floor, insns[index].at, tuple(saved))
