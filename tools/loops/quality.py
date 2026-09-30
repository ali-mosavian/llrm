"""
What an inner loop costs and how it counts, read from the decoded bytes.

Per loop (innerloops.Loop):
- instructions and memory operands, as innerloops counts them;
- induction variables: registers the loop changes only by constant steps and
  uses as an address or in its exit test, plus memory cells it steps so
  (a spilled counter);
- invariant loads: reads whose address the loop never changes and whose cell
  it never stores (a reloaded parameter, a spilled invariant);
- the exit test: whether a cmp/test decides it, and whether the branch reads
  the flags a step left;
- overhead: steps, compares and branches, what the loop pays to go round.
"""

from __future__ import annotations

import sys
from pathlib import Path
from dataclasses import dataclass, field

import iced_x86 as ix

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import innerloops  # noqa: E402

F = ix.FlowControl
WRITES = (ix.OpAccess.WRITE, ix.OpAccess.READ_WRITE, ix.OpAccess.COND_WRITE, ix.OpAccess.READ_COND_WRITE)
READS = (ix.OpAccess.READ, ix.OpAccess.READ_WRITE, ix.OpAccess.COND_READ, ix.OpAccess.READ_COND_WRITE)
STEPS = (ix.Mnemonic.ADD, ix.Mnemonic.SUB, ix.Mnemonic.INC, ix.Mnemonic.DEC, ix.Mnemonic.LEA)
TESTS = (ix.Mnemonic.CMP, ix.Mnemonic.TEST)
_info = ix.InstructionInfoFactory()


@dataclass
class Facts:
    size: int
    memory: int
    calls: int
    ivs: int
    iv_registers: list[str] = field(default_factory=list)
    invariant_loads: int = 0
    compares: int = 0
    branch_after_step: bool = False
    overhead: int = 0
    lines: list[str] = field(default_factory=list)

    def row(self) -> dict:
        return {k: v for k, v in self.__dict__.items() if k != "lines"}


def family(register) -> int:
    if register == ix.Register.NONE:
        return register
    try:
        return ix.RegisterExt.full_register(register)
    except Exception:
        return register


def _name(register) -> str:
    """A register family by its 16-bit name."""
    name = next((n for n in dir(ix.Register) if getattr(ix.Register, n) == register), str(register)).lower()
    return name[1:] if len(name) == 3 and name[0] == "r" and name[1:] in ("ax", "bx", "cx", "dx", "si", "di", "bp", "sp") else name


def _step(one) -> tuple[int, int] | None:
    """(register family, amount) where `one` adds a constant to a register."""
    if one.mnemonic not in STEPS or one.op0_kind != ix.OpKind.REGISTER:
        return None
    target = family(one.op0_register)
    if one.mnemonic == ix.Mnemonic.INC:
        return target, 1
    if one.mnemonic == ix.Mnemonic.DEC:
        return target, -1
    if one.mnemonic == ix.Mnemonic.LEA:
        if family(one.memory_base) == target and one.memory_index == ix.Register.NONE:
            return target, one.memory_displacement
        return None
    if one.op1_kind in (ix.OpKind.IMMEDIATE8, ix.OpKind.IMMEDIATE16, ix.OpKind.IMMEDIATE32, ix.OpKind.IMMEDIATE8TO16,
                        ix.OpKind.IMMEDIATE8TO32):
        value = one.immediate(1)
        return target, value if one.mnemonic == ix.Mnemonic.ADD else -value
    return None


def _trip(body) -> dict[int, int]:
    """Each register's change over one trip, where it is its own value at
    the header plus a constant (followed through copies, adds and lea);
    registers that end as anything else are left out. Branches inside the
    loop are read as falling through."""
    value: dict[int, tuple | None] = {}
    known = lambda reg: value.get(reg, (reg, 0))  # noqa: E731
    for one in body:
        if one.flow_control != F.NEXT:
            continue
        info = _info.info(one)
        targets = {family(u.register) for u in info.used_registers() if u.access in WRITES}
        new = None
        if one.op0_kind == ix.OpKind.REGISTER and one.op0_register != ix.Register.NONE:
            dest = family(one.op0_register)
            width = ix.RegisterExt.size(one.op0_register)
            if width >= 2:
                if one.mnemonic == ix.Mnemonic.MOV and one.op1_kind == ix.OpKind.REGISTER:
                    new = known(family(one.op1_register))
                elif (step := _step(one)) is not None:
                    base = known(dest) if one.mnemonic != ix.Mnemonic.LEA else known(family(one.memory_base))
                    new = (base[0], base[1] + step[1]) if base else None
            for reg in targets:
                value[reg] = None
            if new is not None:
                value[dest] = new
            continue
        for reg in targets:
            value[reg] = None
    return {reg: v[1] for reg, v in value.items() if v is not None and v[0] == reg}


def _memory_step(one) -> tuple | None:
    if one.mnemonic not in STEPS[:4] or one.op0_kind != ix.OpKind.MEMORY:
        return None
    if one.mnemonic in (ix.Mnemonic.ADD, ix.Mnemonic.SUB) and one.op1_kind == ix.OpKind.REGISTER:
        return None
    return _key(one)


def _key(one) -> tuple:
    return (family(one.memory_segment), family(one.memory_base), family(one.memory_index), one.memory_index_scale,
            one.memory_displacement)


def facts(loop: innerloops.Loop) -> Facts:
    if loop.arch != "x86":
        return _msp430(loop)
    body = loop.body
    infos = [_info.info(one) for one in body]
    written: dict[int, list] = {}
    for one, info in zip(body, infos):
        for used in info.used_registers():
            if used.access in WRITES:
                written.setdefault(family(used.register), []).append(one)
    steps = _trip(body)
    stepped = {reg for reg, step in steps.items() if step}
    stores = set()
    for one, info in zip(body, infos):
        for used in info.used_memory():
            if used.access in WRITES:
                stores.add((family(used.segment), family(used.base), family(used.index), used.scale,
                            used.displacement))
    memory_steps = {key for one in body if (key := _memory_step(one))}
    addressed = set()
    for info in infos:
        for used in info.used_memory():
            addressed |= {family(used.base), family(used.index)}
    compared = set()
    for one, info in zip(body, infos):
        if one.mnemonic in TESTS:
            compared |= {family(u.register) for u in info.used_registers() if u.access in READS}
    latch = next((one for one in body if one.ip == loop.latch), body[-1])
    exit_branch = latch if latch.flow_control == F.CONDITIONAL_BRANCH else None
    if exit_branch is None:
        exit_branch = next((one for one in reversed(body) if one.flow_control == F.CONDITIONAL_BRANCH), None)
    flag_setter = None
    if exit_branch is not None:
        at = body.index(exit_branch)
        for one, info in zip(reversed(body[:at]), reversed(infos[:at])):
            if one.rflags_modified:
                flag_setter = one
                break
    exit_regs = set(compared)
    step_of_exit = _step(flag_setter) if flag_setter is not None else None
    if step_of_exit:
        exit_regs.add(step_of_exit[0])
    read = set()
    for info in infos:
        read |= {family(u.register) for u in info.used_registers() if u.access in READS}
    # a register the loop steps and reads: an address, an exit test, a value
    ivs = sorted(reg for reg in stepped if reg in read)
    # invariant registers: never written, or written only by invariant loads
    invariant = lambda reg: reg == ix.Register.NONE or reg not in written  # noqa: E731
    loads = []
    for one, info in zip(body, infos):
        for used in info.used_memory():
            if used.access not in READS or one.mnemonic == ix.Mnemonic.LEA:
                continue
            key = (family(used.segment), family(used.base), family(used.index), used.scale, used.displacement)
            if family(used.base) == family(ix.Register.SP) or key in stores or key in memory_steps:
                continue
            loads.append((one, key))
    changed = True
    inv_loaded: set = set()
    while changed:
        changed = False
        for reg, writers in written.items():
            if reg in inv_loaded:
                continue
            if all(any(w is one and all(invariant(r) or r in inv_loaded for r in key[:3]) for one, key in loads)
                   for w in writers):
                inv_loaded.add(reg)
                changed = True
    invariant_loads = sum(1 for one, key in loads if all(invariant(r) or r in inv_loaded for r in key[:3]))
    compares = sum(1 for one in body if one.mnemonic in TESTS)
    step_count = sum(1 for one in body if _step(one) and _step(one)[0] in ivs) + len(memory_steps)
    step_of_exit = step_of_exit if step_of_exit and step_of_exit[0] in ivs else None
    branches = sum(1 for one in body if one.flow_control in (F.CONDITIONAL_BRANCH, F.UNCONDITIONAL_BRANCH))
    return Facts(
        size=loop.size, memory=loop.memory, calls=loop.calls, ivs=len(ivs) + len(memory_steps),
        iv_registers=[_name(r) for r in ivs] + [f"[{k[1]}+{k[4]}]" for k in memory_steps],
        invariant_loads=invariant_loads, compares=compares,
        branch_after_step=bool(step_of_exit) and step_of_exit[0] in ivs and compares == 0,
        overhead=step_count + compares + branches, lines=loop.lines,
    )


def _msp430(loop) -> Facts:
    """The same counts for LLVM's msp430 loops: a mechanism check only."""
    body = loop.body
    written: dict[str, list] = {}
    for one in body:
        for reg in one.writes:
            written.setdefault(reg, []).append(one)
    stepped = {reg for reg, ws in written.items() if all(reg in w.steps for w in ws)}
    compares = sum(1 for one in body if one.mnemonic in ("cmp", "bit"))
    branches = sum(1 for one in body if one.flow_control in (F.CONDITIONAL_BRANCH, F.UNCONDITIONAL_BRANCH))
    steps = sum(1 for one in body for reg in one.steps if reg in stepped and one.mnemonic in ("add", "sub"))
    return Facts(size=loop.size, memory=loop.memory, calls=loop.calls, ivs=len(stepped), iv_registers=sorted(stepped),
                 compares=compares, overhead=steps + compares + branches, lines=loop.lines)


def function_loops(obj: Path, function: str, procedures: list[str] | None = None) -> list[Facts]:
    """Facts for each innermost loop of `function` (its public or listed name)."""
    found = innerloops.loops(obj.read_bytes(), calls=True, procedures=procedures)
    return [facts(one) for one in found if one.name.rsplit("#", 1)[0] == function]


def bp_problems(text: list) -> list[str]:
    """Where a procedure uses bp as a register (writes it other than as its
    frame), bp must be the frame again before every return, and no frame
    operand ([bp+disp], no index) may be used while it holds something else.
    A point is off the frame if any path reaches it so."""
    if not text or not isinstance(text[0], ix.Instruction):
        return []
    bp = family(ix.Register.BP)
    where = {one.ip: at for at, one in enumerate(text)}
    off = [False] * len(text)  # bp is not the frame on entry to the instruction
    pending = [0]
    seen_entry = set()
    while pending:
        at = pending.pop()
        one = text[at]
        info = _info.info(one)
        state = off[at]
        if bp in {family(u.register) for u in info.used_registers() if u.access in WRITES}:
            restores = one.mnemonic in (ix.Mnemonic.POP, ix.Mnemonic.LEAVE) or (
                one.mnemonic == ix.Mnemonic.MOV and one.op1_kind == ix.OpKind.REGISTER
                and family(one.op1_register) == family(ix.Register.SP))
            state = not restores
        for nxt in innerloops._successors(one, where):
            if nxt not in seen_entry or (state and not off[nxt]):
                seen_entry.add(nxt)
                off[nxt] = off[nxt] or state
                pending.append(nxt)
    problems = []
    for at, one in enumerate(text):
        if not off[at]:
            continue
        for used in _info.info(one).used_memory():
            if family(used.base) == bp and used.index == ix.Register.NONE and one.mnemonic != ix.Mnemonic.LEA:
                problems.append(f"{one.ip:04x}: a frame operand while bp is not the frame")
        if one.flow_control == F.RETURN:
            problems.append(f"{one.ip:04x}: returns while bp is not the frame")
    return problems
