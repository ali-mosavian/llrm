"""
One BC module, as the analysis layer needs to see it.

The move to the object file buys three things the runtime pass could not have.
A call site is a FIXUPP naming an EXTDEF, so "is this B$CPI4" is a lookup
rather than a comparison of a relocated segment and offset. The module's extent
is exact. And an operand's address is a record to read rather than a number
baked into the code -- which it is not: in an object the displacement field
holds zero and the address lives in the fixup.
"""

import struct
from enum import StrEnum
from pathlib import Path
from dataclasses import field
from dataclasses import replace
from dataclasses import dataclass

from qbopt import omf

CALL_FAR = 0x9A


class Space(StrEnum):
    SEGMENT = "seg"  # relocated: an offset into the segment `index` names
    FRAME = "bp"  # bp-relative, so the displacement really is in the code
    LITERAL = "abs"  # a displacement in the code that no fixup claims


@dataclass(frozen=True, slots=True)
class Addr:
    space: Space
    disp: int
    index: int = 0

    def plus(self, bytes_along: int) -> "Addr":
        return replace(self, disp=self.disp + bytes_along)

    def __repr__(self) -> str:
        where = f"{self.space}:{self.index}" if self.space is Space.SEGMENT else self.space
        return f"[{where}{self.disp:+#x}]"


@dataclass(frozen=True, slots=True)
class Module:
    records: list[omf.Record]
    seg: int
    name: str
    code: bytes
    # The predecessor put module-level code after a 0x30-byte header. These
    # fixtures neither confirm nor refute it -- their header fields carry fixups
    # up to 0x20 and the first code operand is at 0x32 -- and starting the lift
    # at 0x30 rather than 0 changes nothing on any of them. So no boundary is
    # claimed here; Phase 6's leaders will decide where code begins.
    start: int
    end: int
    operands: dict[int, Addr] = field(default_factory=dict)
    calls: dict[int, str] = field(default_factory=dict)
    targets: frozenset[int] = frozenset()
    publics: frozenset[int] = frozenset()
    # line-number table entries, which name code offsets like everything else
    lines: frozenset[int] = frozenset()
    # the byte ranges BC split the segment into; a rewrite may not span two
    chunks: tuple[tuple[int, int], ...] = ()
    sites: frozenset[int] = frozenset()
    # the fixup that named each operand field, so a widened form can reuse it
    fixup_at: dict[int, omf.Fixup] = field(default_factory=dict)

    def resolve(self, field_offset: int, literal: int) -> Addr:
        """What the operand whose displacement field sits here points at."""
        return self.operands.get(field_offset, Addr(Space.LITERAL, literal))


def frame_relative(literal: int) -> Addr:
    return Addr(Space.FRAME, literal)


def literal_only(field_offset: int, literal: int) -> Addr:
    """The resolver for code with no fixups behind it, as every unit test has."""
    return Addr(Space.LITERAL, literal)


def of(records: list[omf.Record]) -> Module | None:
    found = omf.code_segment(records)
    if found is None:
        return None
    seg, name, size = found
    code = omf.segment_image(records, seg, size)
    fixups = [fixup for fixup in omf.fixups(records) if fixup.seg == seg]

    operands = {
        fixup.offset: Addr(Space.SEGMENT, fixup.disp, fixup.index)
        for fixup in fixups
        if fixup.loc == omf.LOC_OFF16 and fixup.target in ("segment", "group")
    }
    calls = {
        fixup.offset - 1: omf.externals(records)[fixup.index]
        for fixup in fixups
        if fixup.loc == omf.LOC_PTR32
        and fixup.target == "external"
        and code[fixup.offset - 1 : fixup.offset] == bytes([CALL_FAR])
    }
    targets = frozenset(fixup.disp for fixup in fixups if fixup.target == "segment" and fixup.index == seg)
    named = {
        kind: frozenset(
            struct.unpack_from("<H", record.body, at)[0]
            for record in records
            for at in omf.code_offsets(record, seg)
            if record.type & 0xFE == kind
        )
        for kind in (omf.PUBDEF, omf.LINNUM)
    }

    chunks = tuple(
        (offset, offset + len(payload)) for _record, index, offset, payload in omf.ledata(records) if index == seg
    )
    sites = frozenset(fixup.offset for fixup in fixups)
    fixup_at = {fixup.offset: fixup for fixup in fixups if fixup.offset in operands}

    return Module(
        records,
        seg,
        name,
        code,
        0,
        len(code),
        operands,
        calls,
        targets,
        named[omf.PUBDEF],
        named[omf.LINNUM],
        chunks,
        sites,
        fixup_at,
    )


def load(path: Path | str) -> Module | None:
    return of(omf.read(path))
