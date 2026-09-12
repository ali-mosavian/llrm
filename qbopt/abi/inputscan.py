"""Register inputs and preserved registers discovered from linked OMF definitions.

Each routine in the linked call graph, callees first, gets a summary: the
entry lanes it reads, the registers every return restores, and its stack
effect. Entry values are followed through registers and stack slots, so a
`push si ... pop si` pair is neither a read nor a clobber. A body the model
cannot follow falls back to a lane scan that counts every use as a read and
preserves nothing; unresolved callees read every GP register.

Memory, control and error effects remain the conservative contract supplied
by :mod:`qbopt.abi.runtime`.
"""

from __future__ import annotations

from dataclasses import dataclass

from iced_x86 import OpKind
from iced_x86 import Decoder
from iced_x86 import Mnemonic
from iced_x86 import Register
from iced_x86 import MemorySize
from iced_x86 import FlowControl
from iced_x86 import Instruction
from iced_x86 import MemorySizeExt

from qbopt.abi import runtime
from qbopt.objectfile import omf
from qbopt.frontend.declen import INFO
from qbopt.frontend.declen import READS
from qbopt.frontend.declen import WRITES

type Address = tuple[int, int, int]


class Unrecognized(ValueError):
    """A library routine cannot be decoded into a conservative graph."""


_ROOTS = ("ax", "bx", "cx", "dx", "si", "di")
_LANES = {name: tuple(f"{name}:{byte}" for byte in range(2)) for name in _ROOTS}
_PARTS = {
    Register.EAX: _LANES["ax"],
    Register.AX: _LANES["ax"],
    Register.AL: _LANES["ax"][:1],
    Register.AH: _LANES["ax"][1:],
    Register.EBX: _LANES["bx"],
    Register.BX: _LANES["bx"],
    Register.BL: _LANES["bx"][:1],
    Register.BH: _LANES["bx"][1:],
    Register.ECX: _LANES["cx"],
    Register.CX: _LANES["cx"],
    Register.CL: _LANES["cx"][:1],
    Register.CH: _LANES["cx"][1:],
    Register.EDX: _LANES["dx"],
    Register.DX: _LANES["dx"],
    Register.DL: _LANES["dx"][:1],
    Register.DH: _LANES["dx"][1:],
    Register.ESI: _LANES["si"],
    Register.SI: _LANES["si"],
    Register.EDI: _LANES["di"],
    Register.DI: _LANES["di"],
}
_ALL = frozenset(lane for lanes in _LANES.values() for lane in lanes)


def _symbol(name: str) -> str:
    return name.casefold()


@dataclass(frozen=True, slots=True)
class Input:
    registers: frozenset[runtime.Reg]
    evidence: str


@dataclass(slots=True)
class _Module:
    label: str
    records: tuple[omf.Record, ...]
    definitions: dict[str, tuple[int, int]]
    code_segments: frozenset[int]
    use32_segments: frozenset[int]
    code: dict[int, bytes]
    covered: dict[int, frozenset[int]]
    fixups: tuple
    externals: list[str]


@dataclass(frozen=True, slots=True)
class _Routine:
    address: Address
    instructions: dict[int, object]
    successors: dict[int, tuple[int, ...]]
    calls: dict[int, Address | None]


class Scanner:
    """The definitions visible to one LINK invocation."""

    def __init__(self, members: list[tuple[str, tuple[omf.Record, ...]]]) -> None:
        self.modules: list[_Module] = []
        self.symbols: dict[str, list[Address]] = {}
        self.names: dict[Address, str] = {}
        self._summarised: dict[Address, _Summary] = {}
        self._routines: dict[Address, _Routine] = {}
        for module_index, (label, records) in enumerate(members):
            definitions = omf.public_definitions(list(records))
            code_segments, use32_segments = _segment_kinds(records)
            code, covered = _segments(records)
            found = _Module(
                label,
                records,
                definitions,
                code_segments,
                use32_segments,
                code,
                covered,
                tuple(omf.fixups(list(records))),
                omf.externals(list(records)),
            )
            self.modules.append(found)
            for name, (segment, offset) in definitions.items():
                if segment not in code_segments:
                    continue
                address = (module_index, segment, offset)
                self.symbols.setdefault(_symbol(name), []).append(address)
                self.names.setdefault(address, name)

    def inputs(self, name: str, chosen: tuple[str, int, int] | None = None) -> Input:
        """Inputs of `name`, or every GP input when its graph is incomplete."""
        root = self._root(name, chosen)
        if isinstance(root, str):
            return Input(_registers(_ALL), root)
        summary, count = self._summary(root)
        return Input(
            _registers(summary.inputs),
            f"transitive OMF entry-value scan of {count} routine(s) rooted at "
            f"{self.modules[root[0]].label}:{root[1]}:{root[2]:#x}, following values through "
            "registers and stack slots; recursive and unresolved edges consume all GP inputs",
        )

    def kept(self, name: str, chosen: tuple[str, int, int] | None = None) -> Input:
        """Registers `name` returns as it found them on every path, or none."""
        root = self._root(name, chosen)
        if isinstance(root, str):
            return Input(frozenset(), root)
        summary, count = self._summary(root)
        return Input(
            frozenset(runtime.Reg(one) for one in summary.kept),
            f"transitive OMF save/restore scan of {count} routine(s): every return restores these; "
            "recursive, unresolved and unmodelled stack edges preserve nothing",
        )

    def _summary(self, root: Address) -> tuple[_Summary, int]:
        graph = self._graph(root)
        for address in _cycles(graph):
            self._summarised.setdefault(address, _UNKNOWN)
        pending = {address: routine for address, routine in graph.items() if address not in self._summarised}
        while pending:
            ready = [
                address
                for address, routine in pending.items()
                if all(
                    target is None or target in self._summarised or target not in graph
                    for target in routine.calls.values()
                )
            ]
            if not ready:
                raise Unrecognized("call graph remains cyclic after recursive components were isolated")
            for address in ready:
                self._summarised[address] = self._summarise(pending.pop(address))
        return self._summarised[root], len(graph)

    def _summarise(self, routine: _Routine) -> _Summary:
        try:
            found = _flow(routine, self._summarised)
        except _Opaque:
            found = _Summary(_inputs(routine, self._summarised))
        name = self.names.get(routine.address)
        if name is not None and (documented := runtime.ERROR_FUNNEL_INPUTS.get(name.upper())) is not None:
            found = _Summary(frozenset(lane for register in documented for lane in _LANES[register.value]))
        if name is not None and runtime.never_returns(name):
            # A path into it ends. The error machinery walks frames and
            # return addresses, never a caller's saved register.
            return _Summary(found.inputs, cleanup=0, reach=found.reach or 0, never=True)
        return found

    def _root(self, name: str, chosen: tuple[str, int, int] | None) -> Address | str:
        roots = self.symbols.get(_symbol(name), ())
        if chosen is not None:
            label, segment, offset = chosen
            roots = tuple(
                address
                for address in roots
                if self.modules[address[0]].label == label and address[1:] == (segment, offset)
            )
        return roots[0] if len(roots) == 1 else f"definition scan found {len(roots)} matching entries"

    def _definition(self, name: str) -> Address | None:
        found = self.symbols.get(_symbol(name), ())
        return found[0] if len(found) == 1 else None

    def _target(self, address: Address, insn: Instruction) -> Address | None:
        module_index, segment, _offset = address
        found = self.modules[module_index]
        if insn.flow_control in (FlowControl.INDIRECT_CALL, FlowControl.INDIRECT_BRANCH):
            # A relocation in an indirect transfer identifies the pointer
            # cell, not the code address held in that cell.
            return None
        fixes = [fix for fix in found.fixups if fix.seg == segment and insn.ip <= fix.offset < insn.next_ip]
        if not fixes:
            if insn.op0_kind == OpKind.NEAR_BRANCH16:
                return module_index, segment, insn.near_branch_target
            return None
        if len(fixes) != 1:
            return None
        fix = fixes[0]
        decoder = Decoder(16, found.code[segment][insn.ip :], ip=insn.ip)
        decoded = decoder.decode()
        constants = decoder.get_constant_offsets(decoded)
        width = 4 if fix.loc == 3 else 2
        if (
            fix.offset != insn.ip + constants.immediate_offset
            or constants.immediate_size != 2
            or fix.loc not in (1, 3)
            or any(found.code[segment][fix.offset : fix.offset + width])
        ):
            return None
        if fix.target == "segment":
            if fix.index in found.code_segments:
                return module_index, fix.index, fix.disp
            return None
        if fix.target == "external" and 0 < fix.index < len(found.externals):
            definition = self._definition(found.externals[fix.index])
            if definition is not None:
                module, target_segment, target_offset = definition
                return module, target_segment, target_offset + fix.disp
        return None

    def _decode(self, address: Address) -> _Routine:
        module_index, segment, start = address
        found = self.modules[module_index]
        if segment not in found.code_segments:
            raise Unrecognized(f"{found.label}:{segment}:{start:#x}: control target is not code")
        if segment in found.use32_segments:
            raise Unrecognized(f"{found.label}:{segment}:{start:#x}: USE32 code is not supported")
        code = found.code.get(segment, b"")
        covered = found.covered.get(segment, frozenset())
        instructions = {}
        successors = {}
        calls = {}
        pending = [start]
        while pending:
            at = pending.pop()
            if at in instructions:
                continue
            if len(instructions) >= 4000:
                raise Unrecognized(f"{found.label}:{segment}:{start:#x}: instruction budget exceeded")
            insn = Decoder(16, code[at:], ip=at).decode()
            if insn.is_invalid or not set(range(at, insn.next_ip)) <= covered:
                raise Unrecognized(f"{found.label}:{segment}:{at:#x}: invalid or missing code")
            # A branch may deliberately enter bytes which another path
            # decodes as an immediate. VBDOS lmove.asm does this at 0254/0255
            # (`cmp ax,imm16` versus `mov bx,si`). x86 defines both paths;
            # keying decoded instructions by their entry address represents
            # them without choosing one linear disassembly as authoritative.
            instructions[at] = insn
            next_at: list[int] = []
            match insn.flow_control:
                case FlowControl.NEXT:
                    next_at.append(insn.next_ip)
                case FlowControl.RETURN:
                    pass
                case FlowControl.CALL | FlowControl.INDIRECT_CALL:
                    calls[at] = self._target(address, insn)
                    next_at.append(insn.next_ip)
                case FlowControl.CONDITIONAL_BRANCH | FlowControl.UNCONDITIONAL_BRANCH:
                    target = self._target(address, insn)
                    if target is None:
                        calls[at] = None
                    elif target[:2] != address[:2] or target in self.names and target != address:
                        calls[at] = target
                    else:
                        next_at.append(target[2])
                    if insn.flow_control == FlowControl.CONDITIONAL_BRANCH:
                        next_at.append(insn.next_ip)
                case FlowControl.INDIRECT_BRANCH:
                    calls[at] = None
                case FlowControl.INTERRUPT:
                    calls[at] = None
                    next_at.append(insn.next_ip)
                case _:
                    raise Unrecognized(f"{found.label}:{segment}:{at:#x}: unrecognized control transfer")
            successors[at] = tuple(dict.fromkeys(next_at))
            pending.extend(successors[at])
        return _Routine(address, instructions, successors, calls)

    def _graph(self, root: Address) -> dict[Address, _Routine]:
        graph = {}
        pending = [root]
        while pending:
            address = pending.pop()
            if address in graph:
                continue
            if len(graph) >= 2048:
                raise Unrecognized("function budget exceeded")
            if address not in self._routines:
                self._routines[address] = self._decode(address)
            routine = graph[address] = self._routines[address]
            pending.extend(target for target in routine.calls.values() if target is not None and target not in graph)
        return graph


def _segment_kinds(records: tuple[omf.Record, ...]) -> tuple[frozenset[int], frozenset[int]]:
    """CODE-class and USE32 segment indices declared by SEGDEF records."""
    names = omf.names(list(records))
    code = set()
    use32 = set()
    segment = 0
    for record in records:
        if record.type & 0xFE != omf.SEGDEF:
            continue
        segment += 1
        if not record.body:
            raise Unrecognized(f"segment {segment}: empty SEGDEF")
        attribute = record.body[0]
        if attribute & 1:
            use32.add(segment)
        at = 1 + (4 if record.type & 1 else 2) + (3 if attribute >> 5 == 0 else 0)
        try:
            _, at = omf._index(record.body, at)
            kind, _ = omf._index(record.body, at)
        except IndexError as error:
            raise Unrecognized(f"segment {segment}: malformed SEGDEF") from error
        if kind >= len(names):
            raise Unrecognized(f"segment {segment}: SEGDEF names missing class index {kind}")
        if names[kind].casefold() == "code":
            code.add(segment)
    return frozenset(code), frozenset(use32)


def _segments(records: tuple[omf.Record, ...]) -> tuple[dict[int, bytes], dict[int, frozenset[int]]]:
    pieces: dict[int, list[tuple[int, bytes]]] = {}
    for _record, segment, offset, payload in omf.ledata(list(records)):
        pieces.setdefault(segment, []).append((offset, payload))
    code = {}
    covered = {}
    for segment, chunks in pieces.items():
        end = max(offset + len(payload) for offset, payload in chunks)
        image = bytearray(end)
        seen = set()
        # OMF permits a later LEDATA record to backpatch bytes emitted by an
        # earlier one. Apply file order exactly as LINK does; overlap is not
        # an ambiguity and the covered-byte union remains the same.
        for offset, payload in chunks:
            span = set(range(offset, offset + len(payload)))
            image[offset : offset + len(payload)] = payload
            seen |= span
        code[segment] = bytes(image)
        covered[segment] = frozenset(seen)
    return code, covered


def _inputs(routine: _Routine, summaries: dict[Address, _Summary]) -> frozenset[str]:
    """Entry lanes read on any path before that lane is overwritten."""
    start = routine.address[2]
    incoming: dict[int, frozenset[str]] = {start: _ALL}
    required: set[str] = set()
    pending = [start]
    visits = 0
    while pending:
        visits += 1
        if visits > max(1, len(routine.instructions)) * 100:
            raise Unrecognized(f"{start:#x}: entry-value dataflow did not converge")
        at = pending.pop()
        live = set(incoming[at])
        insn = routine.instructions[at]
        info = INFO.info(insn)
        breaking = (
            insn.mnemonic in (Mnemonic.XOR, Mnemonic.SUB)
            and insn.op_count == 2
            and insn.op0_kind == insn.op1_kind == OpKind.REGISTER
            and insn.op0_register == insn.op1_register
        )
        for used in info.used_registers():
            parts = _PARTS.get(used.register, ())
            if used.access in READS and not (breaking and parts):
                required.update(live.intersection(parts))
            if used.access in WRITES:
                live.difference_update(parts)
        if at in routine.calls:
            target = routine.calls[at]
            wanted = summaries[target].inputs if target in summaries else _ALL
            required.update(live.intersection(wanted))
            # Preservation is intentionally not inferred. Leaving lanes live
            # can only add later entry requirements; clearing them here would
            # silently assume the callee clobbered a value it may preserve.
        for successor in routine.successors.get(at, ()):
            merged = incoming.get(successor, frozenset()) | live
            if merged != incoming.get(successor):
                incoming[successor] = frozenset(merged)
                pending.append(successor)
    return frozenset(required)


@dataclass(frozen=True, slots=True)
class _Summary:
    """What a routine does with its caller's registers and stack.

    `inputs` are entry lanes read, or moved anywhere but back into their own
    place, on some path. `kept` are the roots every return restores.
    `cleanup` is the argument bytes every return pops, `returns` the words
    of return address it pops, and `reach` the words above its entry stack
    pointer it reads or writes, return address included; None when unknown.
    `passes` are the entry lanes that may still be in place at a return.
    """

    inputs: frozenset[str]
    passes: frozenset[str] = _ALL
    kept: frozenset[str] = frozenset()
    cleanup: int | None = None
    returns: int | None = None
    reach: int | None = None
    never: bool = False


# Recursive components and unresolved targets.
_UNKNOWN = _Summary(_ALL)
_FRAME = "frame"
_TRACKED = (*_ROOTS, "bp")
_WORDS = {
    Register.AX: "ax",
    Register.BX: "bx",
    Register.CX: "cx",
    Register.DX: "dx",
    Register.SI: "si",
    Register.DI: "di",
    Register.BP: "bp",
}
_DWORDS = {
    Register.EAX: "ax",
    Register.EBX: "bx",
    Register.ECX: "cx",
    Register.EDX: "dx",
    Register.ESI: "si",
    Register.EDI: "di",
    Register.EBP: "bp",
}
# Each register's root and the bytes of it the register names.
_PLACES = {
    **{
        register: (lanes[0].split(":")[0], tuple(int(lane.split(":")[1]) for lane in lanes))
        for register, lanes in _PARTS.items()
    },
    Register.BP: ("bp", (0, 1)),
    Register.EBP: ("bp", (0, 1)),
}
_STACK = (Register.SP, Register.ESP)
_BASES = (Register.BP, Register.EBP)


class _Opaque(Exception):
    """The stack or a register moves in a way the model does not follow."""


@dataclass(frozen=True, slots=True)
class _Value:
    # What the value certainly is -- a root's entry value, the frame -- or
    # None; and per byte, the entry lanes it may still hold.
    token: object
    lanes: tuple[frozenset[str], frozenset[str]]

    def meet(self, other: _Value) -> _Value:
        return _Value(
            self.token if self.token == other.token else None,
            (self.lanes[0] | other.lanes[0], self.lanes[1] | other.lanes[1]),
        )

    def forget(self) -> _Value:
        return _Value(None, self.lanes)

    def lanes_in(self, places: tuple[int, ...]) -> frozenset[str]:
        return frozenset().union(*(self.lanes[place] for place in places))


_JUNK = _Value(None, (frozenset(), frozenset()))


@dataclass(frozen=True, slots=True)
class _State:
    # Tracked registers in _TRACKED order; word slots pushed since entry;
    # `frame` is the stack depth when bp was set from sp.
    registers: tuple[_Value, ...]
    stack: tuple[_Value, ...]
    frame: int | None

    def get(self, root: str) -> _Value:
        return self.registers[_TRACKED.index(root)]

    def set(self, root: str, value: _Value) -> _State:
        values = list(self.registers)
        values[_TRACKED.index(root)] = value
        # bp holds the frame only while its token says so.
        return _State(tuple(values), self.stack, self.frame if root != "bp" or value.token == _FRAME else None)

    def framed(self) -> bool:
        return self.frame is not None and self.get("bp").token == _FRAME

    def meet(self, other: _State) -> _State:
        if len(self.stack) != len(other.stack):
            raise _Opaque("stack depth differs where paths join")
        return _State(
            tuple(one.meet(two) for one, two in zip(self.registers, other.registers)),
            tuple(one.meet(two) for one, two in zip(self.stack, other.stack)),
            self.frame if self.frame == other.frame else None,
        )


class _Walk:
    """Entry lanes read, and words above entry reached, over one routine."""

    def __init__(self) -> None:
        self.inputs: set[str] = set()
        self.reach: int | None = 0

    def read(self, value: _Value, places: tuple[int, ...] = (0, 1)) -> None:
        self.inputs |= value.lanes_in(places)

    def above(self, words: int | None) -> None:
        if words is None or self.reach is None:
            self.reach = None
        else:
            self.reach = max(self.reach, words)

    def slots(self, state: _State, base: Register, index: Register, displacement: int, size: int) -> list[int] | None:
        """Stack indices a bp-framed operand covers; negative is above entry."""
        if base not in _BASES or not state.framed():
            return None
        if index != Register.NONE:
            raise _Opaque("indexed frame access")
        displacement &= 0xFFFF
        displacement -= 0x10000 if displacement & 0x8000 else 0
        size = max(1, size)
        found = [state.frame - 1 - word for word in range(displacement // 2, (displacement + size - 1) // 2 + 1)]
        if any(index >= len(state.stack) for index in found):
            raise _Opaque("frame access below sp")
        return found

    def memory(self, state: _State, insn: Instruction) -> _State:
        """Frame slots `insn` reads or writes through a bp-framed operand."""
        stack = list(state.stack)
        for memory in INFO.info(insn).used_memory():
            if memory.base in _STACK:
                # Push, pop, call and return are modelled by their callers.
                if insn.mnemonic not in (Mnemonic.PUSH, Mnemonic.POP):
                    raise _Opaque(f"{insn}: addresses the stack through sp")
                continue
            if (
                found := self.slots(
                    state, memory.base, memory.index, memory.displacement, MemorySizeExt.size(memory.memory_size)
                )
            ) is None:
                continue
            for index in found:
                if index < 0:
                    self.above(-index)
                    continue
                if memory.access in READS:
                    self.read(stack[index])
                if memory.access in WRITES:
                    stack[index] = _JUNK
        return _State(state.registers, tuple(stack), state.frame)

    def callee(self, state: _State, summary: _Summary, pushed: int) -> _State:
        """A transfer into `summary`, having pushed `pushed` return words."""
        for lane in summary.inputs:
            root, byte = lane.split(":")
            self.read(state.get(root), (int(byte),))
        # No lane names bp; a callee may still read it.
        self.read(state.get("bp"))
        stack = list(state.stack)
        if summary.reach is None:
            for value in stack:
                self.read(value)
            self.above(None)
            return _State(state.registers, tuple(value.forget() for value in stack), state.frame)
        for word in range(summary.reach - pushed):
            index = len(stack) - 1 - word
            if index < 0:
                self.above(-index)
            else:
                self.read(stack[index])
                stack[index] = stack[index].forget()
        return _State(state.registers, tuple(stack), state.frame)


def _whole(insn: Instruction, operand: int) -> Register:
    if insn.op_count > operand and insn.op_kind(operand) == OpKind.REGISTER:
        return insn.op_register(operand)
    return Register.NONE


def _step(state: _State, insn: Instruction, walk: _Walk) -> _State:
    """One non-transfer instruction's effect on the registers and stack."""
    mnemonic, register, source = insn.mnemonic, _whole(insn, 0), _whole(insn, 1)
    if state.framed() and (
        source in _BASES
        and not (mnemonic == Mnemonic.MOV and register == Register.SP)
        or register in _BASES
        and mnemonic not in (Mnemonic.PUSH, Mnemonic.POP, Mnemonic.MOV)
    ):
        raise _Opaque(f"{insn}: the frame's address escapes")
    if (
        mnemonic == Mnemonic.LEA
        and (found := walk.slots(state, insn.memory_base, insn.memory_index, insn.memory_displacement, 2)) is not None
    ):
        # A pointer to the caller's arguments reaches an unknown extent of
        # them; one into this routine's own slots is not followed.
        if max(found) >= 0:
            raise _Opaque(f"{insn}: the frame's address escapes")
        walk.above(None)
    match mnemonic:
        case Mnemonic.PUSH:
            if register in _DWORDS:
                value = state.get(_DWORDS[register])
                return _State(
                    state.registers, (*state.stack, _Value(("high", value.token), _JUNK.lanes), value), state.frame
                )
            if register in _WORDS:
                return _State(state.registers, (*state.stack, state.get(_WORDS[register])), state.frame)
            state = walk.memory(state, insn)
            return _State(
                state.registers, (*state.stack, *((_JUNK,) * (-insn.stack_pointer_increment // 2))), state.frame
            )
        case Mnemonic.POP:
            words = insn.stack_pointer_increment // 2
            if len(state.stack) < words:
                raise _Opaque("pop below entry")
            popped, rest = state.stack[len(state.stack) - words :], state.stack[: len(state.stack) - words]
            after = _State(state.registers, rest, state.frame)
            if register in _DWORDS:
                high, low = popped
                return after.set(_DWORDS[register], low if high.token == ("high", low.token) else low.forget())
            if register in _WORDS:
                return after.set(_WORDS[register], popped[-1])
            if register in _STACK:
                raise _Opaque("pop sp")
            if insn.op0_kind != OpKind.REGISTER:
                if walk.slots(state, insn.memory_base, insn.memory_index, insn.memory_displacement, 2) is not None:
                    raise _Opaque(f"{insn}: pops into the frame")
                for base in (insn.memory_base, insn.memory_index):
                    if (place := _PLACES.get(base)) is not None:
                        walk.read(state.get(place[0]), place[1])
            # Into memory or a segment register: the value is used.
            for value in popped:
                walk.read(value)
            return after
        case Mnemonic.PUSHA | Mnemonic.PUSHAD:
            slots = (
                [state.get(root) for root in ("ax", "cx", "dx", "bx")]
                + [_JUNK]
                + [state.get(root) for root in ("bp", "si", "di")]
            )
            width = 2 if mnemonic == Mnemonic.PUSHAD else 1
            return _State(
                state.registers,
                (*state.stack, *(slot for one in slots for slot in ((_JUNK,) * (width - 1) + (one,)))),
                state.frame,
            )
        case Mnemonic.POPA | Mnemonic.POPAD:
            width = 2 if mnemonic == Mnemonic.POPAD else 1
            if len(state.stack) < 8 * width:
                raise _Opaque("popa below entry")
            words = state.stack[len(state.stack) - 8 * width :][width - 1 :: width]
            after = _State(state.registers, state.stack[: len(state.stack) - 8 * width], state.frame)
            for root, value in zip(("di", "si", "bp", None, "bx", "dx", "cx", "ax"), words):
                if root is not None:
                    after = after.set(root, value)
            return after
        case Mnemonic.PUSHF | Mnemonic.PUSHFD:
            return _State(
                state.registers, (*state.stack, *((_JUNK,) * (2 if mnemonic == Mnemonic.PUSHFD else 1))), state.frame
            )
        case Mnemonic.POPF | Mnemonic.POPFD:
            count = 2 if mnemonic == Mnemonic.POPFD else 1
            if len(state.stack) < count:
                raise _Opaque("popf below entry")
            for value in state.stack[-count:]:
                walk.read(value)
            return _State(state.registers, state.stack[:-count], state.frame)
        case Mnemonic.LEAVE:
            if not state.framed():
                raise _Opaque("leave without a known frame")
            unwound = _State(state.registers, state.stack[: state.frame], None)
            return _step(unwound, Decoder(16, b"\x5d").decode(), walk)
        case Mnemonic.ENTER if insn.immediate8_2nd == 0:
            pushed = _step(state, Decoder(16, b"\x55").decode(), walk)
            framed = _State(pushed.set("bp", _Value(_FRAME, _JUNK.lanes)).registers, pushed.stack, len(pushed.stack))
            return _State(framed.registers, (*framed.stack, *((_JUNK,) * (insn.immediate16 // 2))), framed.frame)
        case Mnemonic.MOV if register == Register.BP and source == Register.SP:
            return _State(state.set("bp", _Value(_FRAME, _JUNK.lanes)).registers, state.stack, len(state.stack))
        case Mnemonic.MOV if register == Register.SP and source == Register.BP:
            if not state.framed():
                raise _Opaque("sp restored from an unknown frame")
            return _State(state.registers, state.stack[: state.frame], state.frame)
        case Mnemonic.ADD | Mnemonic.SUB if register == Register.SP and insn.op1_kind in (
            OpKind.IMMEDIATE8TO16,
            OpKind.IMMEDIATE16,
        ):
            amount = insn.immediate(1) & 0xFFFF
            amount = amount - 0x10000 if amount & 0x8000 else amount
            words, odd = divmod(amount if mnemonic == Mnemonic.SUB else -amount, 2)
            if odd:
                raise _Opaque("sp moved by an odd amount")
            if words >= 0:
                return _State(state.registers, (*state.stack, *((_JUNK,) * words)), state.frame)
            if len(state.stack) < -words:
                raise _Opaque("sp released below entry")
            return _State(state.registers, state.stack[:words], state.frame)
        case Mnemonic.MOV if register in _WORDS and source in _WORDS:
            return state.set(_WORDS[register], state.get(_WORDS[source]))
        case Mnemonic.XCHG if register in _WORDS and source in _WORDS:
            one, two = _WORDS[register], _WORDS[source]
            return state.set(one, state.get(two)).set(two, state.get(one))
        case Mnemonic.MOV if (register in _WORDS or source in _WORDS) and insn.memory_size == MemorySize.UINT16:
            # A word moved between a register and its own frame slot.
            found = walk.slots(state, insn.memory_base, insn.memory_index, insn.memory_displacement, 2)
            if found is not None and len(found) == 1 and found[0] >= 0:
                (index,) = found
                if register in _WORDS:
                    return state.set(_WORDS[register], state.stack[index])
                stack = list(state.stack)
                stack[index] = state.get(_WORDS[source])
                return _State(state.registers, tuple(stack), state.frame)
    breaking = (
        mnemonic in (Mnemonic.XOR, Mnemonic.SUB)
        and insn.op_count == 2
        and register != Register.NONE
        and register == source
    )
    used = INFO.info(insn).used_registers()
    for one in used:
        if one.register in _STACK:
            raise _Opaque(f"{insn}: uses sp")
        if one.access in READS and not breaking and (place := _PLACES.get(one.register)) is not None:
            walk.read(state.get(place[0]), place[1])
    after = walk.memory(state, insn)
    for one in used:
        if one.access in WRITES and (place := _PLACES.get(one.register)) is not None:
            root, places = place
            lanes = tuple(frozenset() if byte in places else lanes for byte, lanes in enumerate(after.get(root).lanes))
            after = after.set(root, _Value(None, lanes))
    return after


def _flow(routine: _Routine, summaries: dict[Address, _Summary]) -> _Summary:
    """Follow entry values through registers and stack slots to every exit."""
    start = routine.address[2]
    entry = tuple(_Value(root, (frozenset({f"{root}:0"}), frozenset({f"{root}:1"}))) for root in _ROOTS)
    # Keyed by depth too: paths may meet at different depths where what
    # follows never returns (B$STALC's two ways into B$ERR_OS).
    states: dict[tuple[int, int], _State] = {(start, 0): _State((*entry, _Value("bp", _JUNK.lanes)), (), None)}
    walk = _Walk()
    preserved: frozenset[str] | None = None
    passes: set[str] = set()
    exits: set[tuple[int | None, int | None]] = set()
    pending = [(start, 0)]
    visits = 0

    def finish(state: _State, cleanup: int | None, returns: int | None) -> None:
        nonlocal preserved
        if state.stack:
            raise _Opaque("returns with its own words still on the stack")
        for root in _TRACKED:
            value = state.get(root)
            for byte in range(2):
                walk.inputs |= value.lanes[byte] - {f"{root}:{byte}"}
                passes.update(value.lanes[byte] & {f"{root}:{byte}"})
        here = frozenset(root for root in _TRACKED if state.get(root).token == root)
        preserved = here if preserved is None else preserved & here
        exits.add((cleanup, returns))

    while pending:
        visits += 1
        if visits > max(1, len(routine.instructions)) * 100:
            raise _Opaque("did not converge")
        key = pending.pop()
        at, state = key[0], states[key]
        insn = routine.instructions[at]
        flow = insn.flow_control
        if flow == FlowControl.RETURN:
            if insn.mnemonic not in (Mnemonic.RET, Mnemonic.RETF):
                raise _Opaque(f"{insn}: not a plain return")
            finish(state, insn.immediate16 if insn.op_count else 0, 2 if insn.mnemonic == Mnemonic.RETF else 1)
            continue
        if at in routine.calls:
            target = routine.calls[at]
            summary = summaries.get(target, _UNKNOWN) if target is not None else _UNKNOWN
            if flow == FlowControl.INTERRUPT:
                for value in state.registers:
                    walk.read(value)
                state = _State(tuple(value.forget() for value in state.registers), state.stack, None)
            else:
                far = insn.op0_kind in (OpKind.FAR_BRANCH16, OpKind.FAR_BRANCH32)
                pushed = (2 if far else 1) if flow in (FlowControl.CALL, FlowControl.INDIRECT_CALL) else 0
                called = walk.callee(state, summary, pushed)
                registers = tuple(
                    value
                    if root in summary.kept
                    else _Value(
                        None,
                        tuple(
                            lanes if root == "bp" or f"{root}:{byte}" in summary.passes else frozenset()
                            for byte, lanes in enumerate(value.lanes)
                        ),
                    )
                    for root, value in zip(_TRACKED, called.registers)
                )
                after = _State(registers, called.stack, called.frame if registers[-1].token == _FRAME else None)
                if summary.never:
                    # Only a conditional branch has a path past it.
                    if flow != FlowControl.CONDITIONAL_BRANCH:
                        continue
                elif pushed:
                    if summary.cleanup is None or summary.returns is None or summary.cleanup % 2:
                        raise _Opaque(f"{insn}: callee's stack effect is not known")
                    words = summary.cleanup // 2 + summary.returns - pushed
                    if words < 0 or len(after.stack) < words:
                        raise _Opaque(f"{insn}: callee pops more than was pushed")
                    state = _State(after.registers, after.stack[: len(after.stack) - words], after.frame)
                else:
                    # A jump into another routine returns from there.
                    finish(after, summary.cleanup, summary.returns)
        else:
            state = _step(state, insn, walk)
        for successor in routine.successors.get(at, ()):
            key = successor, len(state.stack)
            merged = state if key not in states else states[key].meet(state)
            if merged != states.get(key):
                states[key] = merged
                pending.append(key)
    cleanup, returns = next(iter(exits)) if len(exits) == 1 else (None, None)
    return _Summary(frozenset(walk.inputs), frozenset(passes), preserved or frozenset(), cleanup, returns, walk.reach)


def _cycles(graph: dict[Address, _Routine]) -> set[Address]:
    """Every node in a recursive component; those components stay opaque."""
    reachable = {}
    for start in graph:
        seen = set()
        pending = [target for target in graph[start].calls.values() if target in graph]
        while pending:
            target = pending.pop()
            if target in seen:
                continue
            seen.add(target)
            pending.extend(child for child in graph[target].calls.values() if child in graph and child not in seen)
        reachable[start] = seen
    return {address for address, seen in reachable.items() if address in seen}


def _registers(lanes: frozenset[str]) -> frozenset[runtime.Reg]:
    return frozenset(runtime.Reg(name) for name, parts in _LANES.items() if lanes.intersection(parts))
