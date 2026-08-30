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

from iced_x86 import Register
from iced_x86 import Register_

from qbopt import omf

CALL_FAR = 0x9A

# BC's own linker directive segment group. Measured: the same 11-segment set
# in every one of the 110 real objects (BC_CN, BC_DATA, BC_DS, BC_FT, BC_SA,
# BC_SAB, BR_DATA, BR_SKYS, COMMON, ENMALLOC, NMALLOC).
DGROUP = "DGROUP"


class Space(StrEnum):
    SEGMENT = "seg"  # relocated: an offset into the segment `index` names
    FRAME = "bp"  # bp-relative, so the displacement really is in the code
    LITERAL = "abs"  # a displacement in the code that no fixup claims
    # relocated against a GRPDEF rather than a SEGDEF -- a different index
    # namespace than SEGMENT's. Measured: 0 of 13,292 code-segment fixups
    # across all 110 real objects target a group; only "segment" appears.
    # Kept distinct rather than folded into SEGMENT so nothing ever compares
    # a group index against a segment index by accident -- resolving which
    # segments a group covers is out of scope, so an address in this space
    # is refused everywhere it matters: may_alias() answers True (never
    # provably disjoint), lift.operand() refuses to resolve one at all, and
    # lift.memory() raises if one ever reaches it regardless.
    GROUP = "grp"


# base is only ever one of these two -- operand() in lift.py sets nothing
# else -- so a name outside the pair prints as itself rather than nothing.
INDEX_NAMES = {Register.SI: "si", Register.DI: "di"}


@dataclass(frozen=True, slots=True)
class Addr:
    space: Space
    disp: int
    index: int = 0
    # NONE for a bare displacement; an array element also carries the
    # register its offset was indexed by, since two elements at the same
    # displacement are not the same address unless that register agrees too.
    base: Register_ = Register.NONE

    def plus(self, bytes_along: int) -> "Addr":
        return replace(self, disp=self.disp + bytes_along)

    def __repr__(self) -> str:
        where = f"{self.space}:{self.index}" if self.space is Space.SEGMENT else self.space
        indexed = f"+{INDEX_NAMES.get(self.base, f'r{self.base}')}" if self.base != Register.NONE else ""
        return f"[{where}{indexed}{self.disp:+#x}]"


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
    # segment indices DGROUP's own GRPDEF names -- a stack slot (Space.FRAME)
    # can never be the same byte as a segment outside this set, which is what
    # may_alias() rests on.
    dgroup: frozenset[int] = frozenset()

    def resolve(self, field_offset: int, literal: int) -> Addr:
        """What the operand whose displacement field sits here points at."""
        return self.operands.get(field_offset, Addr(Space.LITERAL, literal))


def frame_relative(literal: int) -> Addr:
    return Addr(Space.FRAME, literal)


def literal_only(field_offset: int, literal: int) -> Addr:
    """The resolver for code with no fixups behind it, as every unit test has."""
    return Addr(Space.LITERAL, literal)


# The widest access anything here can name -- an x87 qword load. Over-stating
# an access's width only ever makes two ranges overlap that would not have, so
# it is the answer a caller that does not know its own width should get.
WIDEST = 8


def _overlaps(a: Addr, a_width: int, b: Addr, b_width: int) -> bool:
    """Whether [disp, disp+width) intersect -- arithmetic, not analysis."""
    return a.disp < b.disp + b_width and b.disp < a.disp + a_width


def may_alias(
    a: Addr | None,
    b: Addr | None,
    dgroup: frozenset[int],
    a_width: int = WIDEST,
    b_width: int = WIDEST,
) -> bool:
    """Whether two addresses could name the same byte, conservatively.

    False only where it is provable from the object alone, which -- measured
    across the corpus -- is most of the time: 94% of explicit memory
    references resolve through a fixup to an exact (segment, displacement),
    so disjointness between two of them is arithmetic on the displacements
    and needs no assumption whatsoever. The cases, in the order they are
    decided:

    An indexed address is never provably disjoint from anything. `[si+arr]`
    with si unbounded can reach any byte of its segment, and bounding it
    needs array extents the object does not carry -- so an indexed operand
    reads or writes its whole segment as far as this is concerned. That is
    76 instructions corpus-wide, which is what makes refusing them cheap.

    Two bare displacements are disjoint when their own ranges do not meet.
    Within one object a SEGDEF index names one segment, and two distinct
    SEGDEFs are two distinct segments, so a differing index is disjoint
    outright; a matching one is the range test. Same for two frame slots,
    where the ranges are bp-relative -- sound only while bp is invariant
    across the region asking, which is the caller's own obligation to check
    (nothing here can see whether something wrote bp), the way registers.py
    already checks a single register's own liveness.

    A frame slot against a segment DGROUP never lists can never be the same
    byte, because DGROUP is exactly the set SS is assumed to overlap. That
    assumption -- SS==DS, so a frame slot and a DGROUP segment address might
    coincide -- cannot be *proven* from the object: SS itself does not exist
    until the runtime sets it up at link/load time. It is centralised here
    rather than re-derived in prose at every call site, and it is the one
    rule here that rests on anything beyond arithmetic.

    None stands for "address not known" -- an unresolved operand, or a
    Space.GROUP address (see Space.GROUP's own comment) -- and is never
    provably disjoint from anything.
    """
    if a is None or b is None:
        return True
    if a.base != Register.NONE or b.base != Register.NONE:
        return True
    match (a.space, b.space):
        case (Space.FRAME, Space.FRAME):
            return _overlaps(a, a_width, b, b_width)
        case (Space.SEGMENT, Space.SEGMENT):
            return a.index == b.index and _overlaps(a, a_width, b, b_width)
        case (Space.FRAME, Space.SEGMENT) if b.index not in dgroup:
            return False
        case (Space.SEGMENT, Space.FRAME) if a.index not in dgroup:
            return False
        case _:
            return True


def of(records: list[omf.Record]) -> Module | None:
    found = omf.code_segment(records)
    if found is None:
        return None
    seg, name, size = found
    code = omf.segment_image(records, seg, size)
    fixups = [fixup for fixup in omf.fixups(records) if fixup.seg == seg]

    operands = {
        fixup.offset: Addr(Space.SEGMENT if fixup.target == "segment" else Space.GROUP, fixup.disp, fixup.index)
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
    dgroup = frozenset(omf.groups(records).get(DGROUP, ()))

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
        dgroup,
    )


def load(path: Path | str) -> Module | None:
    return of(omf.read(path))
