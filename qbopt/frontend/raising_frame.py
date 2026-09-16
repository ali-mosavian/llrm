"""Prove argument-stack accesses disjoint from unescaped main-frame locals.

Runtime-specific geometry and machine stack tracking end here. Consumers see
only byte-range exclusions on individual memory effects.
"""

from dataclasses import replace

from iced_x86 import OpKind
from iced_x86 import Mnemonic
from iced_x86 import Register
from iced_x86 import FlowControl

from qbopt.model import mir
from qbopt.abi import runtime
from qbopt.objectfile import module
from qbopt.frontend.declen import INFO
from qbopt.frontend.extent import ENTRY
from qbopt.frontend.declen import WRITES
from qbopt.frontend.extent import has_header
from qbopt.frontend.blocks import event_enabled

FIXED = {module.Family.QUICKBASIC: 10, module.Family.PDS: 18, module.Family.VBDOS: 20}
POINTERS = frozenset((Register.SP, Register.ESP, Register.BP, Register.EBP))
UNSEEN = object()


def _layout(found):
    fixed = FIXED.get(module.family(found.records))
    if fixed is None or not has_header(found):
        return None
    size = int.from_bytes(found.code[0x22:0x24], "little")
    if not 0 < size < 0x8000 - fixed:
        return None
    return -fixed - size, size


def _private(blocks):
    for block in blocks:
        for decoded in block.insns:
            insn = decoded.insn
            if any(
                insn.op_kind(index) == OpKind.REGISTER and insn.op_register(index) in POINTERS
                for index in range(insn.op_count)
            ):
                return False
            if insn.mnemonic == Mnemonic.LEA and insn.memory_base in POINTERS:
                return False
    return True


def _after(decoded, depth, contracts):
    if depth is None:
        return None
    insn = decoded.insn
    if insn.flow_control == FlowControl.INTERRUPT:
        return None
    if insn.mnemonic == Mnemonic.CALL:
        contract = contracts.get(decoded.at)
        if (
            contract is None
            or not contract.established
            or runtime.barrier(contract)
            or contract.cleanup is None
            or contract.control is not runtime.Control.RETURNS
            or contract.clobbers & {runtime.Reg.SP, runtime.Reg.BP}
        ):
            return None
        return depth + contract.cleanup
    writes = {used.register for used in INFO.info(insn).used_registers() if used.access in WRITES}
    if writes & {Register.BP, Register.EBP, Register.SS}:
        return None
    if insn.mnemonic == Mnemonic.POP and insn.op0_register in (Register.SP, Register.ESP):
        return None
    if writes & {Register.SP, Register.ESP} and insn.mnemonic not in (Mnemonic.PUSH, Mnemonic.POP):
        return None
    return depth + insn.stack_pointer_increment


def _depths(blocks, entry, initial, contracts):
    predecessors = {block.at: [] for block in blocks}
    for block in blocks:
        for successor in block.succ:
            if successor in predecessors:
                predecessors[successor].append(block.at)
    exits = dict.fromkeys(predecessors, UNSEEN)
    entries = {}
    changed = True
    while changed:
        changed = False
        for block in blocks:
            incoming = [exits[at] for at in predecessors[block.at] if exits[at] is not UNSEEN]
            if block.at == entry:
                incoming.append(initial)
            if not incoming:
                continue
            depth = incoming[0] if all(one == incoming[0] for one in incoming) else None
            entries[block.at] = depth
            for decoded in block.insns:
                depth = _after(decoded, depth, contracts)
                if depth is not None and not -0x10000 < depth <= initial:
                    depth = None
            if exits[block.at] != depth:
                exits[block.at] = depth
                changed = True
    result = {}
    for block in blocks:
        depth = entries.get(block.at)
        for decoded in block.insns:
            result[decoded.at] = depth
            depth = _after(decoded, depth, contracts)
            if depth is not None and not -0x10000 < depth <= initial:
                depth = None
    return result


def annotated(body, found, blocks, contracts):
    layout = _layout(found)
    if body.entry != ENTRY or layout is None or event_enabled(found) or runtime.handles_errors(contracts.values()):
        return body
    floor, size = layout
    depths = _depths(blocks, body.entry, floor, contracts)
    private = _private(blocks)
    decoded = {one.at: one for block in blocks for one in block.insns}
    exclusion = (module.Addr(module.Space.FRAME, floor), size)

    def rewrite(op):
        depth = depths.get(op.at)
        if depth is None or (one := decoded.get(op.at)) is None:
            return op
        insn = one.insn
        stack = insn.mnemonic in (Mnemonic.PUSH, Mnemonic.POP)
        after = _after(one, depth, contracts)
        if after is None or not -0x10000 < min(depth, after) <= max(depth, after) <= floor:
            return op
        contract = contracts.get(op.at)
        own = (
            private
            and op.kind is mir.Kind.CALL
            and contract is not None
            and contract.writes is runtime.Memory.OWN
            and contract.reads is runtime.Memory.OWN
            and contract.inputs is not None
            and not contract.inputs & {runtime.Reg.BP, runtime.Reg.SP}
        )
        if not stack and not own:
            return op

        implicit = op.stores if insn.mnemonic == Mnemonic.PUSH else op.loads if stack else ()
        implicit = frozenset(
            ref
            for ref in implicit
            if ref.base is None and ref.segment is None and (ref.addr is None or ref.addr.space is module.Space.STACK)
        )

        def reference(ref):
            if ref in implicit or (own and ref.addr is None):
                return replace(ref, excludes=tuple(dict.fromkeys((*ref.excludes, exclusion))))
            return ref

        def argument(arg):
            return mir.Cell(reference(arg.ref)) if isinstance(arg, mir.Cell) else arg

        return replace(
            op,
            loads=tuple(map(reference, op.loads)),
            stores=tuple(map(reference, op.stores)),
            args=tuple(map(argument, op.args)),
            results=tuple(map(argument, op.results)),
        )

    return replace(body, blocks=tuple(replace(block, ops=tuple(map(rewrite, block.ops))) for block in body.blocks))
