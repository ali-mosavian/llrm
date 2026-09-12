"""The compiler's near-to-far event-poll adapter, recognized before MIR."""

from dataclasses import replace
from typing import TYPE_CHECKING

from iced_x86 import Code

from qbopt.objectfile import omf
from qbopt.frontend import blocks
from qbopt.objectfile import module

if TYPE_CHECKING:
    from qbopt.abi.runtime import Contract


def handler_entries(found: module.Module) -> frozenset[int]:
    """Handlers passed by the established TIMER registration sequence."""
    from qbopt.abi.handlers import registered

    return registered(found, "B$ONTA", ("pds71", "vbdos"))


def contracts(found: module.Module) -> "dict[int, Contract]":
    from qbopt.abi import runtime

    family = module.family(found.records)
    routine = runtime.VARIANTS.get(("B$EVK1", family))
    if routine is None or "B$EVK1" in module.defines(found.records, found.seg):
        return {}
    start = blocks.event_stub(found)
    if start is None:
        return {}
    fields = [fixup for fixup in omf.fixups(found.records) if fixup.seg == found.seg]
    mapped = blocks.code_map(found)
    if isinstance(mapped, str):
        return {}
    contract = replace(
        routine,
        evidence=(
            "BC event adapter: CMP EVTFLG/JNE/RET; POP AX/PUSH CS/PUSH AX/"
            "JMP FAR B$EVK1 converts the near return address to a far one. "
            "No caller arguments; all event effects retained. " + routine.evidence
        ),
    )
    return {
        one.at: contract
        for block in blocks.partition(found, mapped)
        for one in block.insns
        if one.insn.code == Code.CALL_REL16
        and one.insn.near_branch_target == start
        and not any(one.at <= fixup.offset < one.end for fixup in fields)
    }
