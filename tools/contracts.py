"""Inspect 16-bit OMF function contracts and their transitive dependencies.

    uv run python tools/contracts.py 'B$FreeHandleBlock' --lib path/to/runtime.lib

This is an evidence tool, not an ABI declaration generator. Unknown edges,
recursion and unsupported stack operations suppress preservation proofs.
"""

import sys
import json
import argparse
from pathlib import Path
from dataclasses import field
from dataclasses import asdict
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
from iced_x86 import Formatter
from iced_x86 import Register_
from iced_x86 import FlowControl
from iced_x86 import RegisterExt
from iced_x86 import FormatterSyntax

from qbopt import omf
from qbopt.declen import INFO
from qbopt.declen import READS
from qbopt.declen import WRITES

REGISTERS = ("ax", "bx", "cx", "dx", "si", "di", "bp", "ds", "es", "ss", "fs", "gs", "flags")
WORDS = {getattr(Register, name.upper()): name for name in REGISTERS if name != "flags"}
ROOTS = {RegisterExt.full_register(reg): name for reg, name in WORDS.items()}
FORMAT = Formatter(FormatterSyntax.NASM)
type Address = tuple[int, int, int]


def register_name(reg: Register_) -> str | None:
    return ROOTS.get(RegisterExt.full_register(reg))


@dataclass
class Routine:
    address: Address
    name: str
    instructions: dict = field(default_factory=dict)
    successors: dict = field(default_factory=dict)
    calls: dict = field(default_factory=dict)
    unknown: list[str] = field(default_factory=list)


@dataclass
class Contract:
    reads: list[str]
    clobbers: list[str]
    preserved: list[str]
    restored: list[str]
    memory_write: bool
    cleanup: int | None
    unknown: list[str]


def opaque(reason: str) -> Contract:
    return Contract(list(REGISTERS), list(REGISTERS), [], [], True, None, [reason])


class Library:
    def __init__(self, objects: list[Module], limit: int = 2000, functions: int = 256) -> None:
        self.objects = objects
        self.limit = limit
        self.functions = functions
        self.symbols: dict[str, list[Address]] = {}
        self.names = {}
        self.codes = {}
        self.covered = {}
        self.relocations = {}
        for index, module in enumerate(objects):
            if any(record.type in omf.WIDE for record in module.records):
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
        segments = [record for record in self.objects[index].records if record.type == omf.SEGDEF]
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
        return routine

    def graph(self, root: Address) -> dict[Address, Routine]:
        graph = {}
        pending = [root]
        while pending and len(graph) < self.functions:
            address = pending.pop()
            if address in graph:
                continue
            routine = graph[address] = self.decode(address)
            pending.extend(target for target, _ in routine.calls.values() if target is not None and target not in graph)
        return graph


def analyze(routine: Routine, contracts: dict[Address, Contract], budget: int = 10000) -> Contract:
    """Path-sensitive entry-value tokens, with conservative memory aliasing.

    Reads include save/pass-through reads, not merely semantic arguments.
    Only whole 16-bit register moves and stack pushes/pops preserve tokens.
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
            name = register_name(used.register)
            if name and used.access in READS:
                reads.add(name)
            if name and used.access in WRITES:
                written.add(name)
                values[name] = "?"
                if name == "ss":
                    unknown.add(f"{at:04x}: stack segment change")
        if insn.rflags_read:
            reads.add("flags")
        if insn.rflags_modified:
            written.add("flags")
            values["flags"] = "?"
        destination = WORDS.get(insn.op0_register) if insn.op0_kind == OpKind.REGISTER else None
        source = WORDS.get(insn.op1_register) if insn.op_count > 1 and insn.op1_kind == OpKind.REGISTER else None
        push = insn.mnemonic == Mnemonic.PUSH and insn.stack_pointer_increment == -2
        pop = insn.mnemonic == Mnemonic.POP and destination is not None and insn.stack_pointer_increment == 2
        call = insn.flow_control in (FlowControl.CALL, FlowControl.INDIRECT_CALL)
        returning = insn.mnemonic in (Mnemonic.RET, Mnemonic.RETF)
        if push:
            stack += (before[destination] if destination else "?",)
        elif pop:
            if stack:
                values[destination] = stack[-1]
                stack = stack[:-1]
            else:
                unknown.add(f"{at:04x}: pop outside tracked stack")
        elif insn.mnemonic == Mnemonic.MOV and destination and source:
            values[destination] = before[source]
        elif insn.mnemonic == Mnemonic.MOV and destination == "bp" and insn.op1_register == Register.SP:
            values["bp"] = f"stack:{len(stack)}"
        elif (
            insn.mnemonic == Mnemonic.MOV
            and insn.op0_register == Register.SP
            and source == "bp"
            and before["bp"].startswith("stack:")
        ):
            depth = int(before["bp"].split(":")[1])
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
            memory_write |= callee.memory_write
            if callee.memory_write:
                stack = tuple("?" for _ in stack)
            if callee.unknown:
                unknown.add(f"{at:04x}: dependency incomplete ({'; '.join(callee.unknown[:2])[:200]})")
            if call:
                if callee.cleanup is None or callee.cleanup % 2 or callee.cleanup > len(stack) * 2:
                    unknown.add(f"{at:04x}: unmodeled callee stack cleanup")
                elif callee.cleanup:
                    stack = stack[: -callee.cleanup // 2]
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
    return contracts


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("symbol")
    parser.add_argument(
        "--lib", type=Path, action="append", required=True, help="OMF .lib or .obj; repeat for dependencies"
    )
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--dump", type=Path, help="write reachable disassembly and contracts to JSON")
    parser.add_argument("--instructions", type=int, default=2000)
    parser.add_argument("--functions", type=int, default=256)
    args = parser.parse_args()
    if min(args.instructions, args.functions) <= 0:
        parser.error("budgets must be positive")
    library = Library(
        [module for path in args.lib for module in modules(path.read_bytes())], args.instructions, args.functions
    )
    roots = library.symbols.get(args.symbol, [])
    if len(roots) != 1:
        parser.error(f"expected one definition of {args.symbol}, found {len(roots)}")
    graph = library.graph(roots[0])
    contracts = summarize(graph)
    report = {
        "scope": "16-bit GP/segment registers and aggregate flags; reads include saves; upper halves/x87 unproved; "
        "SP described by cleanup only; conditional on normal return with immutable code, "
        "not termination or exception safety; memory aliasing conservative",
        "root": library.label(roots[0]),
        "inputs": [str(path.resolve()) for path in args.lib],
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
                **asdict(contracts[address]),
                "dependencies": {
                    f"{at:04x}": library.label(target) if target else reason
                    for at, (target, reason) in routine.calls.items()
                },
                "disassembly": [
                    f"{at:04x}: {library.codes[address[:2]][at:insn.next_ip].hex():<16} {FORMAT.format(insn)}"
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
