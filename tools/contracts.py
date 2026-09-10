"""Inspect 16-bit OMF function contracts and their transitive dependencies.

    uv run python tools/contracts.py 'B$FreeHandleBlock' --lib path/to/runtime.lib

This is an evidence tool, not an ABI declaration generator. Unknown edges,
recursion and unsupported stack operations suppress preservation proofs.
"""

import sys
import json
import argparse
from pathlib import Path
from hashlib import sha256
from dataclasses import field
from dataclasses import asdict
from dataclasses import replace
from dataclasses import dataclass

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from libdump import Module
from iced_x86 import OpKind
from libdump import code_of
from libdump import modules
from iced_x86 import Decoder
from iced_x86 import Mnemonic
from iced_x86 import Register
from iced_x86 import RflagsBits
from iced_x86 import Formatter
from iced_x86 import Register_
from iced_x86 import FlowControl
from iced_x86 import FormatterSyntax

from qbopt.objectfile import omf
from qbopt.frontend.declen import INFO
from qbopt.frontend.declen import READS
from qbopt.frontend.declen import WRITES

FLAG_BITS = {
    f"flags:{name.lower()}": getattr(RflagsBits, name)
    for name in ("OF", "SF", "ZF", "AF", "CF", "PF", "DF", "IF", "AC", "UIF", "C0", "C1", "C2", "C3")
}
ALIASES: dict[str, tuple[str, ...]] = {"flags": tuple(FLAG_BITS)}
ALIASES.update({lane.split(":")[1]: (lane,) for lane in FLAG_BITS})
for _word in ("ax", "bx", "cx", "dx", "si", "di", "bp"):
    _lanes = tuple(f"e{_word}:{byte}" for byte in range(4))
    ALIASES["e" + _word] = _lanes
    ALIASES[_word] = _lanes[:2]
    ALIASES[f"e{_word}[31:16]"] = _lanes[2:]
    if _word[1] == "x":
        ALIASES[_word[0] + "l"] = _lanes[:1]
        ALIASES[_word[0] + "h"] = _lanes[1:2]
for _segment in ("ds", "es", "ss", "fs", "gs"):
    ALIASES[_segment] = tuple(f"{_segment}:{byte}" for byte in range(2))
PARTS = {getattr(Register, name.upper()): lanes for name, lanes in ALIASES.items() if hasattr(Register, name.upper())}
REGISTERS = tuple(dict.fromkeys(lane for lanes in ALIASES.values() for lane in lanes))
FORMAT = Formatter(FormatterSyntax.NASM)
type Address = tuple[int, int, int]


def register_parts(reg: Register_) -> tuple[str, ...]:
    return PARTS.get(reg, ())


@dataclass
class Routine:
    address: Address
    name: str
    instructions: dict = field(default_factory=dict)
    successors: dict = field(default_factory=dict)
    calls: dict = field(default_factory=dict)
    unknown: list[str] = field(default_factory=list)
    excluded_edges: dict[int, str] = field(default_factory=dict)


@dataclass
class Contract:
    reads: list[str]
    clobbers: list[str]
    preserved: list[str]
    restored: list[str]
    memory_write: bool
    cleanup: int | None
    unknown: list[str]
    flag_values: dict[str, int] = field(default_factory=dict)


def opaque(reason: str) -> Contract:
    return Contract(list(REGISTERS), list(REGISTERS), [], [], True, None, [reason])


class Library:
    def __init__(self, objects: list[Module], limit: int = 2000, functions: int | None = 256) -> None:
        self.objects = objects
        self.limit = limit
        self.functions = functions
        self.symbols: dict[str, list[Address]] = {}
        self.names = {}
        self.codes = {}
        self.covered = {}
        self.relocations = {}
        for index, module in enumerate(objects):
            if any(record.type in omf.WIDE - {omf.SEGDEF + 1} for record in module.records):
                raise ValueError(f"{module.name}: 32-bit OMF records are not supported")
            for name, (seg, offset) in module.defines().items():
                address = (index, seg, offset)
                self.symbols.setdefault(name, []).append(address)
                self.names.setdefault(address, name)
            self.relocations[index] = omf.fixups(module.records)
            for _, seg, offset, payload in omf.ledata(module.records):
                self.covered.setdefault((index, seg), set()).update(range(offset, offset + len(payload)))

    def label(self, address: Address) -> str:
        index, seg, offset = address
        return f"[{index}]{self.objects[index].name}:{seg}:{offset:04x}:{self.names.get(address, '(local)')}"

    def target(self, address: Address, insn) -> tuple[Address | None, str]:
        index, seg, _ = address
        if insn.flow_control in (FlowControl.INDIRECT_CALL, FlowControl.INDIRECT_BRANCH):
            return None, "indirect control transfer"
        fixes = [f for f in self.relocations[index] if f.seg == seg and insn.ip <= f.offset < insn.next_ip]
        if fixes:
            if len(fixes) != 1:
                return None, "multiple control relocations"
            fix = fixes[0]
            code = self.codes[index, seg]
            decoder = Decoder(16, code[insn.ip :], ip=insn.ip)
            decoded = decoder.decode()
            constants = decoder.get_constant_offsets(decoded)
            # Only the offset16 and pointer16:16 forms with zero addends
            # used by these libraries are resolved. Never use placeholder bytes.
            if (
                fix.offset != insn.ip + constants.immediate_offset
                or constants.immediate_size != 2
                or fix.loc not in (1, 3)
                or any(code[fix.offset : fix.offset + (4 if fix.loc == 3 else 2)])
            ):
                return None, "unsupported relocation/addend"
            if fix.target == "segment":
                return (index, fix.index, fix.disp), ""
            if fix.target == "external":
                externals = omf.externals(self.objects[index].records)
                name = externals[fix.index] if 0 < fix.index < len(externals) else "invalid EXTDEF"
                matches = self.symbols.get(name, [])
                if len(matches) == 1:
                    module, segment, offset = matches[0]
                    return (module, segment, offset + fix.disp), ""
                return None, f"{name}: {'ambiguous' if matches else 'unresolved'} external"
            return None, f"unsupported {fix.target} relocation"
        if insn.op0_kind == OpKind.NEAR_BRANCH16:
            return (index, seg, insn.near_branch_target), ""
        return None, "unrelocated far control transfer"

    def decode(self, address: Address) -> Routine:
        index, seg, start = address
        key = index, seg
        code = self.codes.setdefault(key, code_of(self.objects[index], seg))
        routine = Routine(address, self.label(address))
        segments = [record for record in self.objects[index].records if record.type & 0xFE == omf.SEGDEF]
        if 0 < seg <= len(segments) and segments[seg - 1].body[0] & 1:
            routine.unknown.append("USE32 segment is not supported")
            return routine
        pending = [start]
        while pending:
            at = pending.pop()
            if at in routine.instructions:
                continue
            if len(routine.instructions) >= self.limit:
                routine.unknown.append("instruction budget exceeded")
                break
            insn = Decoder(16, code[at:], ip=at).decode()
            if insn.is_invalid or not set(range(at, insn.next_ip)) <= self.covered.get(key, set()):
                routine.unknown.append(f"{at:04x}: invalid or missing code")
                continue
            routine.instructions[at] = insn
            if any(
                other.ip != at and other.ip < insn.next_ip and at < other.next_ip
                for other in routine.instructions.values()
            ):
                routine.unknown.append(f"{at:04x}: overlapping instructions")
            successors = []
            match insn.flow_control:
                case FlowControl.NEXT:
                    successors = [insn.next_ip]
                case FlowControl.RETURN:
                    if insn.mnemonic not in (Mnemonic.RET, Mnemonic.RETF):
                        routine.unknown.append(f"{at:04x}: unsupported return")
                case FlowControl.CALL | FlowControl.INDIRECT_CALL:
                    routine.calls[at] = self.target(address, insn)
                    successors = [insn.next_ip]
                case FlowControl.CONDITIONAL_BRANCH | FlowControl.UNCONDITIONAL_BRANCH | FlowControl.INDIRECT_BRANCH:
                    target, reason = self.target(address, insn)
                    if target is not None and target[:2] == key and (target not in self.names or target == address):
                        successors.append(target[2])
                    else:
                        routine.calls[at] = target, reason
                    if insn.flow_control == FlowControl.CONDITIONAL_BRANCH:
                        successors.append(insn.next_ip)
                case _:
                    routine.unknown.append(f"{at:04x}: unsupported control transfer")
            routine.successors[at] = successors
            pending.extend(successors)
        relocated = {
            insn.ip
            for insn in routine.instructions.values()
            if any(fix.seg == seg and insn.ip <= fix.offset < insn.next_ip for fix in self.relocations[index])
        }
        return constant_paths(routine, relocated)

    def graph(self, root: Address) -> dict[Address, Routine]:
        return self.graph_from([root])

    def code_entries(self, classes: set[str]) -> list[Address]:
        code = set()
        for index, module in enumerate(self.objects):
            names = omf.names(module.records)
            segment = 0
            for record in module.records:
                if record.type & 0xFE != omf.SEGDEF:
                    continue
                segment += 1
                at = 1 + (4 if record.type & 1 else 2) + (3 if record.body[0] >> 5 == 0 else 0)
                _, at = omf._index(record.body, at)
                kind, _ = omf._index(record.body, at)
                if kind < len(names) and names[kind].upper() in classes:
                    code.add((index, segment))
        return sorted(address for address in self.names if address[:2] in code)

    def graph_from(self, roots: list[Address]) -> dict[Address, Routine]:
        graph = {}
        pending = list(reversed(roots))
        while pending and (self.functions is None or len(graph) < self.functions):
            address = pending.pop()
            if address in graph:
                continue
            routine = graph[address] = self.decode(address)
            pending.extend(target for target, _ in routine.calls.values() if target is not None and target not in graph)
        return graph


def constant_paths(routine: Routine, relocated: set[int]) -> Routine:
    """Meet byte facts at joins; only exclude a path when its ZF is established.

    Calls invalidate all facts here. Context-sensitive callee analysis is a
    separate obligation, not an assumed ABI. Relocated immediates are unknown.
    """
    entry = routine.address[2]
    incoming: dict[int, dict[str, int]] = {entry: {}}
    pending = [entry]
    edges = {}
    calls = {}
    excluded = {}
    steps = 0
    while pending:
        steps += 1
        if steps > max(1, len(routine.instructions)) * 100:
            return routine
        at = pending.pop()
        insn = routine.instructions.get(at)
        if insn is None:
            continue
        before = incoming[at]
        facts = constant_step(insn, before, at in relocated)
        successors = routine.successors.get(at, [])
        taken = None
        if "zero" in before and insn.mnemonic in (Mnemonic.JE, Mnemonic.JNE):
            taken = bool(before["zero"]) == (insn.mnemonic == Mnemonic.JE)
            if at in routine.calls or any(target != insn.next_ip for target in successors):
                successors = [target for target in successors if (target != insn.next_ip) == taken]
            excluded[at] = f"ZF={before['zero']}; branch {'taken' if taken else 'not taken'}"
        else:
            excluded.pop(at, None)
        calls.pop(at, None)
        if at in routine.calls and taken is not False:
            calls[at] = routine.calls[at]
            facts = {}
        edges[at] = successors
        for target in successors:
            old = incoming.get(target)
            merged = (
                facts.copy()
                if old is None
                else {name: value for name, value in old.items() if facts.get(name) == value}
            )
            if old is None or old != merged:
                incoming[target] = merged
                pending.append(target)
    return replace(
        routine,
        instructions={at: insn for at, insn in routine.instructions.items() if at in incoming},
        successors=edges,
        calls=calls,
        excluded_edges=excluded,
    )


def constant_step(insn, before: dict[str, int], relocated: bool) -> dict[str, int]:
    facts = before.copy()
    for used in INFO.info(insn).used_registers():
        if used.access in WRITES:
            for part in register_parts(used.register):
                facts.pop(part, None)
    if insn.rflags_modified:
        facts.pop("zero", None)
    if relocated:
        return facts
    destination = register_parts(insn.op0_register) if insn.op0_kind == OpKind.REGISTER else ()
    if not destination or insn.op_count != 2:
        return facts
    left = (
        sum(before[part] << (8 * byte) for byte, part in enumerate(destination))
        if all(part in before for part in destination)
        else None
    )
    right = None
    if insn.op1_kind == OpKind.REGISTER:
        source = register_parts(insn.op1_register)
        if source and all(part in before for part in source):
            right = sum(before[part] << (8 * byte) for byte, part in enumerate(source))
    elif insn.op1_kind in (
        OpKind.IMMEDIATE8,
        OpKind.IMMEDIATE16,
        OpKind.IMMEDIATE32,
        OpKind.IMMEDIATE8TO16,
        OpKind.IMMEDIATE8TO32,
    ):
        right = insn.immediate(1) & ((1 << (len(destination) * 8)) - 1)
    answer = None
    match insn.mnemonic:
        case Mnemonic.MOV:
            answer = right
        case Mnemonic.XOR | Mnemonic.SUB if insn.op1_kind == OpKind.REGISTER and insn.op0_register == insn.op1_register:
            answer = 0
            facts["zero"] = 1
        case Mnemonic.CMP if left is not None and right is not None:
            facts["zero"] = int(left == right)
        case Mnemonic.TEST if left is not None and right is not None:
            facts["zero"] = int((left & right) == 0)
    if answer is not None:
        facts.update({part: (answer >> (8 * byte)) & 255 for byte, part in enumerate(destination)})
    return facts


def analyze(routine: Routine, contracts: dict[Address, Contract], budget: int = 10000) -> Contract:
    """Path-sensitive byte-lane entry tokens, with conservative memory aliasing.

    Reads include save/pass-through reads, not merely semantic arguments.
    Register moves and word/dword stack pushes/pops preserve matching lanes.
    Arbitrary stores invalidate saved stack tokens: SS can alias DS/ES.
    """
    unknown = set(routine.unknown)
    reads, written = set(), set()
    memory_write = False
    returns = []
    pending: list[tuple[int, tuple[str, ...], tuple[str, ...]]] = [(routine.address[2], tuple(REGISTERS), ())]
    seen = set()
    while pending:
        state = pending.pop()
        if state in seen:
            continue
        if len(seen) >= budget:
            unknown.add("analysis state budget exceeded")
            break
        seen.add(state)
        at, tokens, stack = state
        insn = routine.instructions.get(at)
        if insn is None:
            unknown.add(f"{at:04x}: missing successor")
            continue
        values = dict(zip(REGISTERS, tokens, strict=True))
        before = values.copy()
        info = INFO.info(insn)
        for used in info.used_registers():
            parts = register_parts(used.register)
            if used.access in READS:
                reads.update(parts)
            if used.access in WRITES:
                written.update(parts)
                for part in parts:
                    values[part] = "?"
                if used.register == Register.SS:
                    unknown.add(f"{at:04x}: stack segment change")
        for lane, bit in FLAG_BITS.items():
            if insn.rflags_read & bit:
                reads.add(lane)
            if insn.rflags_modified & bit:
                written.add(lane)
                values[lane] = "?"
            if insn.rflags_cleared & bit:
                values[lane] = "0"
            if insn.rflags_set & bit:
                values[lane] = "1"
        destination = register_parts(insn.op0_register) if insn.op0_kind == OpKind.REGISTER else ()
        source = register_parts(insn.op1_register) if insn.op_count > 1 and insn.op1_kind == OpKind.REGISTER else ()
        width = abs(insn.stack_pointer_increment)
        push = insn.mnemonic == Mnemonic.PUSH and width in (2, 4)
        pop = insn.mnemonic == Mnemonic.POP and len(destination) == width and width in (2, 4)
        call = insn.flow_control in (FlowControl.CALL, FlowControl.INDIRECT_CALL)
        returning = insn.mnemonic in (Mnemonic.RET, Mnemonic.RETF)
        if push:
            stack += tuple(before[part] for part in destination) if len(destination) == width else ("?",) * width
        elif pop:
            if len(stack) >= width:
                values.update(zip(destination, stack[-width:], strict=True))
                stack = stack[:-width]
            else:
                unknown.add(f"{at:04x}: pop outside tracked stack")
        elif insn.mnemonic == Mnemonic.MOV and destination and source:
            if len(destination) == len(source):
                values.update(zip(destination, (before[part] for part in source), strict=True))
        elif insn.mnemonic == Mnemonic.MOV and insn.op0_register == Register.BP and insn.op1_register == Register.SP:
            for part in ALIASES["bp"]:
                values[part] = f"stack:{len(stack)}"
        elif (
            insn.mnemonic == Mnemonic.MOV
            and insn.op0_register == Register.SP
            and insn.op1_register == Register.BP
            and before[ALIASES["bp"][0]].startswith("stack:")
            and before[ALIASES["bp"][0]] == before[ALIASES["bp"][1]]
        ):
            depth = int(before[ALIASES["bp"][0]].split(":")[1])
            if depth <= len(stack):
                stack = stack[:depth]
            else:
                unknown.add(f"{at:04x}: frame outside tracked stack")
        elif (
            any(
                used.register in (Register.SP, Register.ESP, Register.RSP) and used.access in WRITES
                for used in info.used_registers()
            )
            and not call
            and not returning
        ):
            unknown.add(f"{at:04x}: unmodeled stack change")
            stack = tuple("?" for _ in stack)
        stores = any(mem.access in WRITES for mem in info.used_memory())
        if stores and not push and not call:
            memory_write = True
            stack = tuple("?" for _ in stack)
        if at in routine.calls:
            target, reason = routine.calls[at]
            callee = contracts.get(target, opaque(reason or "recursive or budget-limited dependency"))
            reads.update(callee.reads)
            written.update(callee.clobbers)
            for name in callee.clobbers:
                values[name] = "?"
            values.update({name: str(value) for name, value in callee.flag_values.items()})
            memory_write |= callee.memory_write
            if callee.memory_write:
                stack = tuple("?" for _ in stack)
            if callee.unknown:
                unknown.add(f"{at:04x}: dependency incomplete ({'; '.join(callee.unknown[:2])[:200]})")
            if call:
                if callee.cleanup is None or callee.cleanup > len(stack):
                    unknown.add(f"{at:04x}: unmodeled callee stack cleanup")
                elif callee.cleanup:
                    stack = stack[: -callee.cleanup]
            else:
                if stack:
                    unknown.add(f"{at:04x}: tail transfer with pending stack saves")
                returns.append((values, callee.cleanup))
        if returning:
            if stack:
                unknown.add(f"{at:04x}: unbalanced stack at return")
            cleanup = insn.immediate16 if insn.op_count else 0
            frame = 4 if insn.mnemonic == Mnemonic.RETF else 2
            if insn.stack_pointer_increment != frame + cleanup:
                unknown.add(f"{at:04x}: unsupported return width")
            returns.append((values, cleanup))
        if len(stack) > 128:
            unknown.add(f"{at:04x}: stack tracking budget exceeded")
            continue
        pending.extend(
            (next_at, tuple(values[name] for name in REGISTERS), stack) for next_at in routine.successors.get(at, [])
        )
    if not returns:
        unknown.add("no modeled return")
    preserved = {name for name in REGISTERS if returns and all(values[name] == name for values, _ in returns)}
    if unknown:
        preserved.clear()
        reads.update(REGISTERS)
        memory_write = True
    cleanups = {cleanup for _, cleanup in returns}
    return Contract(
        sorted(reads),
        sorted(set(REGISTERS) - preserved),
        sorted(preserved),
        sorted(preserved & written),
        memory_write,
        next(iter(cleanups)) if len(cleanups) == 1 and not unknown else None,
        sorted(unknown),
        {
            lane: int(returns[0][0][lane])
            for lane in FLAG_BITS
            if not unknown
            and returns
            and returns[0][0][lane] in ("0", "1")
            and all(values[lane] == returns[0][0][lane] for values, _ in returns)
        },
    )


def cycles(graph: dict[Address, Routine]) -> list[set[Address]]:
    reachable = {}
    for start in graph:
        seen = set()
        pending = [target for target, _ in graph[start].calls.values() if target in graph]
        while pending:
            target = pending.pop()
            if target in seen:
                continue
            seen.add(target)
            pending.extend(child for child, _ in graph[target].calls.values() if child in graph and child not in seen)
        reachable[start] = seen
    groups = []
    assigned = set()
    for start, seen in reachable.items():
        if start in seen and start not in assigned:
            group = {target for target in seen if start in reachable[target]}
            groups.append(group)
            assigned.update(group)
    return groups


def summarize(graph: dict[Address, Routine]) -> dict[Address, Contract]:
    recursive = set().union(*cycles(graph))
    contracts = {address: opaque("recursive dependency cycle") for address in recursive}
    pending = {address: routine for address, routine in graph.items() if address not in recursive}
    while pending:
        ready = [
            address
            for address, routine in pending.items()
            if all(target is None or target in contracts or target not in graph for target, _ in routine.calls.values())
        ]
        assert ready, "cycles must be separated before topological propagation"
        for address in ready:
            contracts[address] = analyze(pending.pop(address), contracts)
    # Retain local diagnostics, without feeding a circular preservation claim
    # back into itself. Every cyclic member keeps an opaque summary as input.
    details = {address: analyze(graph[address], contracts) for address in recursive}
    contracts.update(details)
    return {address: alias_contract(contract) for address, contract in contracts.items()}


def alias_contract(contract: Contract) -> Contract:
    preserved = set(contract.preserved)

    def overlaps(parts: list[str]) -> list[str]:
        return sorted(name for name, lanes in ALIASES.items() if set(lanes).intersection(parts))

    return Contract(
        overlaps(contract.reads),
        overlaps(contract.clobbers),
        sorted(name for name, lanes in ALIASES.items() if set(lanes) <= preserved),
        sorted(
            name
            for name, lanes in ALIASES.items()
            if set(lanes) <= preserved and set(lanes).intersection(contract.restored)
        ),
        contract.memory_write,
        contract.cleanup,
        contract.unknown,
        {lane.split(":")[1]: value for lane, value in contract.flag_values.items()},
    )


def grouped_registers(registers: list[str]) -> dict[str, list[str]]:
    """Present overlapping aliases together without dropping partial effects."""
    selected = set(registers)
    groups: dict[str, list[str]] = {}
    for name, lanes in sorted(
        ALIASES.items(), key=lambda item: ("[" in item[0], -len(item[1]), item[0].endswith("l"), item[0])
    ):
        if name not in selected:
            continue
        family = lanes[0].split(":")[0]
        if family in ("ds", "es", "ss", "fs", "gs"):
            family = "segments"
        groups.setdefault(family, []).append(name)
    order = ("eax", "ebx", "ecx", "edx", "esi", "edi", "ebp", "segments", "flags")
    return {family: groups[family] for family in order if family in groups}


def contract_report(contract: Contract) -> dict:
    report = asdict(contract)
    for attribute in ("reads", "clobbers", "preserved", "restored"):
        report[attribute] = grouped_registers(report[attribute])
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("symbol", nargs="?")
    parser.add_argument("--all", action="store_true", help="analyze every public code entry and reachable helper")
    parser.add_argument("--code-class", action="append", help="code segment class (default CODE)")
    parser.add_argument(
        "--lib", type=Path, action="append", required=True, help="OMF .lib or .obj; repeat for dependencies"
    )
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--dump", type=Path, help="write reachable disassembly and contracts to JSON")
    parser.add_argument("--instructions", type=int, default=2000)
    parser.add_argument("--functions", type=int, help="default 256 for one symbol; unlimited for --all")
    args = parser.parse_args()
    if args.instructions <= 0 or (args.functions is not None and args.functions <= 0):
        parser.error("budgets must be positive")
    if bool(args.symbol) == args.all:
        parser.error("supply either a symbol or --all")
    inputs = [(path, path.read_bytes()) for path in args.lib]
    library = Library(
        [module for _, data in inputs for module in modules(data)],
        args.instructions,
        args.functions if args.functions is not None else (None if args.all else 256),
    )
    roots = (
        library.code_entries({name.upper() for name in args.code_class or ["CODE"]})
        if args.all
        else library.symbols.get(args.symbol, [])
    )
    if not args.all and len(roots) != 1:
        parser.error(f"expected one definition of {args.symbol}, found {len(roots)}")
    if not roots:
        parser.error("no public entries in selected code classes")
    graph = library.graph_from(roots)
    contracts = summarize(graph)
    report = {
        "schema_version": 3,
        "scope": "8/16/32-bit overlapping GP registers, data segments and decoder-modeled flag bits; "
        "reads include saves; x87 unproved; "
        "SP described by cleanup only; conditional on normal return with immutable code, "
        "not termination or exception safety; memory aliasing conservative",
        "root": None if args.all else library.label(roots[0]),
        "roots": [library.label(address) for address in roots],
        "unvisited_roots": [library.label(address) for address in roots if address not in graph],
        "excluded_public_entries": [library.label(address) for address in library.names if address not in roots]
        if args.all
        else [],
        "inputs": [str(path.resolve()) for path in args.lib],
        "input_sha256": {str(path.resolve()): sha256(data).hexdigest() for path, data in inputs},
        "recursive_components": [[library.label(address) for address in sorted(group)] for group in cycles(graph)],
        "unvisited_dependencies": sorted(
            {
                library.label(target)
                for routine in graph.values()
                for target, _ in routine.calls.values()
                if target is not None and target not in graph
            }
        ),
        "functions": {
            routine.name: {
                **contract_report(contracts[address]),
                "dependencies": {
                    f"{at:04x}": library.label(target) if target else reason
                    for at, (target, reason) in routine.calls.items()
                },
                "excluded_edges": {f"{at:04x}": evidence for at, evidence in routine.excluded_edges.items()},
                "disassembly": [
                    f"{at:04x}: {library.codes[address[:2]][at : insn.next_ip].hex():<16} {FORMAT.format(insn)}"
                    for at, insn in sorted(routine.instructions.items())
                ],
            }
            for address, routine in graph.items()
        },
    }
    output = json.dumps(report, indent=2)
    if args.dump:
        args.dump.write_text(output + "\n")
    if args.json:
        print(output)
    else:
        print(report["scope"])
        if args.all:
            complete = sum(not contract.unknown for contract in contracts.values())
            print(
                f"{len(roots)} public code entries; {len(graph)} unique routines; "
                f"{complete} modeled, {len(graph) - complete} incomplete"
            )
            print(
                f"{len(report['unvisited_roots'])} unvisited roots; "
                f"{len(report['unvisited_dependencies'])} unvisited dependencies"
            )
            if args.dump:
                print(f"Full contract map: {args.dump}")
                return 0
        for name, contract in report["functions"].items():
            print(f"\n{name}")
            for key in (
                "reads",
                "clobbers",
                "preserved",
                "restored",
                "memory_write",
                "cleanup",
                "dependencies",
                "unknown",
            ):
                print(f"  {key}: {contract[key]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
