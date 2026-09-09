"""The compiler's near-to-far event-poll adapter, recognized before MIR."""

from dataclasses import replace
from typing import TYPE_CHECKING

from iced_x86 import Code

from qbopt.frontend import blocks
from qbopt.objectfile import module, omf

if TYPE_CHECKING:
    from qbopt.abi.runtime import Contract


def handler_entries(found: module.Module) -> frozenset[int]:
    """Handlers passed by the established TIMER registration sequence."""
    if module.family(found.records) not in ("pds71", "vbdos"):
        return frozenset()
    if "B$ONTA" in module.defines(found.records, found.seg):
        return frozenset()
    entries = set()
    for at, name in found.calls.items():
        if name != "B$ONTA" or at < 5:
            continue
        match found.code[at - 5:at]:
            case b"\xb8\x00\x00\x0e\x50":
                field = at - 4
            case b"\x0e\xb8\x00\x00\x50":
                field = at - 3
            case _:
                continue
        ref = found.operands.get(field)
        if (ref is not None and ref.space is module.Space.SEGMENT
                and ref.index == found.seg and found.start <= ref.disp < found.end):
            entries.add(ref.disp)
    return frozenset(entries)


def contracts(found: module.Module) -> "dict[int, Contract]":
    from qbopt.abi import runtime

    family = module.family(found.records)
    routine = runtime.VARIANTS.get(("B$EVK1", family))
    if routine is None or "B$EVK1" in module.defines(found.records, found.seg):
        return {}
    start = blocks.ENTRY
    adapter = bytes.fromhex("eb10 833e000000 7501 c3 58 0e 50 ea00000000")
    if not blocks.event_enabled(found) or found.code[start:start + len(adapter)] != adapter:
        return {}
    fields = [fixup for fixup in omf.fixups(found.records) if fixup.seg == found.seg]
    names = omf.externals(found.records)
    expected = {
        start + 4: (omf.LOC_OFF16, "b$EVTFLG"),
        start + 14: (omf.LOC_PTR32, "B$EVK1"),
    }
    inside = [fixup for fixup in fields if start <= fixup.offset < start + len(adapter)]
    if len(inside) != len(expected):
        return {}
    for fixup in inside:
        if (fixup.target != "external" or fixup.disp != 0 or fixup.selfrel
                or expected.get(fixup.offset) != (fixup.loc, names[fixup.index])):
            return {}
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
        and one.insn.near_branch_target == start + 2
        and not any(one.at <= fixup.offset < one.end for fixup in fields)
    }
