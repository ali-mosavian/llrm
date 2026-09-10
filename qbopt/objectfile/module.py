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

from qbopt.objectfile import omf

CALL_FAR = 0x9A

# BC's own linker directive segment group. Measured: the same 11-segment set
# in every one of the 110 real objects (BC_CN, BC_DATA, BC_DS, BC_FT, BC_SA,
# BC_SAB, BR_DATA, BR_SKYS, COMMON, ENMALLOC, NMALLOC).
DGROUP = "DGROUP"


class Space(StrEnum):
    SEGMENT = "seg"  # relocated: an offset into the segment `index` names
    EXTERNAL = "external"  # relocated against EXTDEF; distinct symbols may alias
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
    # a $DYNAMIC array element: `es:[bx]`, `es:[bx+2]`. There is no fixup
    # behind it -- bx holds a byte offset BC computed at run time and es
    # holds whatever a prior `mov es,[desc+2]` last loaded, so disp is a
    # literal in the code exactly as Space.FRAME's is, and `segment` names
    # which register the offset is read through. Measured against
    # qb-qrender: 11,150 segment-override instructions, all of them exactly
    # this shape (`mov`, `cmp`, `push`, `add`, `and`, `idiv`, `sub`, `imul`,
    # `sbb`, always `es:[bx]` or `es:[bx+2]`, never another base or another
    # override register) -- so base is BX and segment is a real register on
    # every Addr this space ever holds, not just in principle.
    FAR = "far"
    # a slot the code itself pushed, addressed by how far sp has moved since
    # the top of the block that pushed it -- so `disp` is a depth, not an
    # address, and only two STACK addresses from the same block are ever
    # compared. Everything else aliases it, deliberately: BC's SS==DS means
    # a pushed slot and a frame local or a DGROUP static could coincide, and
    # ruling that out needs sp's relation to bp, which nothing here tracks.
    # Enough to link a push to the pop that reads it, which is what the
    # round-trip idiom absorption emits is made of.
    STACK = "sp"


# base is si/di for a SEGMENT array element and bx for a FAR one -- operand()
# in lift.py sets nothing else -- so a name outside this set prints as itself
# rather than nothing.
INDEX_NAMES = {Register.SI: "si", Register.DI: "di", Register.BX: "bx"}
SEGMENT_NAMES = {Register.ES: "es", Register.DS: "ds", Register.SS: "ss", Register.CS: "cs"}


@dataclass(frozen=True, slots=True)
class Addr:
    space: Space
    disp: int
    index: int = 0
    # NONE for a bare displacement; an array element also carries the
    # register its offset was indexed by, since two elements at the same
    # displacement are not the same address unless that register agrees too.
    # A FAR address's own offset register (always bx, measured) lives here.
    base: Register_ = Register.NONE
    # NONE everywhere except Space.FAR, where it is the segment register the
    # access actually goes through -- part of the address's own identity,
    # since `es:[bx]` and `ds:[bx]` are not the same byte merely because bx
    # agrees. See Space.FAR's own comment.
    segment: Register_ = Register.NONE

    def plus(self, bytes_along: int) -> "Addr":
        return replace(self, disp=self.disp + bytes_along)

    def __repr__(self) -> str:
        if self.space is Space.FAR:
            seg = SEGMENT_NAMES.get(self.segment, f"r{self.segment}")
            base = INDEX_NAMES.get(self.base, f"r{self.base}")
            return f"[{seg}:{base}{self.disp:+#x}]"
        where = f"{self.space}:{self.index}" if self.space in (Space.SEGMENT, Space.EXTERNAL) else self.space
        indexed = f"+{INDEX_NAMES.get(self.base, f'r{self.base}')}" if self.base != Register.NONE else ""
        return f"[{where}{indexed}{self.disp:+#x}]"


class Family(StrEnum):
    """Which compiler made an object.

    Not a preference: B$ENRA reads bx under VBDOS's runtime and does not
    touch it under PDS's, so a routine's contract is not one fact for
    every toolchain. Read from COMENT class 0x00, which every object
    carries and which names the compiler outright.
    """

    QUICKBASIC = "qb45"
    PDS = "pds71"
    VBDOS = "vbdos"
    UNKNOWN = "unknown"


_MADE_BY = (
    (b"QuickBASIC Compiler 4.5", Family.QUICKBASIC),
    (b"BASIC Compiler 7.1", Family.PDS),
    (b"VBDOS", Family.VBDOS),
)


def defines(records: "list[omf.Record]", seg: int) -> "frozenset[str]":
    """Every name this module declares itself: BC compiles a SUB as a
    PUBDEF and calls it through an EXTDEF fixup of the same object, so the
    call target alone cannot say whether it is the runtime's or its own."""
    return frozenset(omf.pubdef_names(records, seg).values())


def family(records: "list[omf.Record]") -> Family:
    """Which compiler wrote these records, from its own comment."""
    for one in records:
        if one.type != 0x88 or len(one.body) < 2 or one.body[1] != 0x00:
            continue
        said = one.body[2:]
        for mark, which in _MADE_BY:
            if mark in said:
                return which
    return Family.UNKNOWN


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
    # The segment BC put this program's own variables in -- the one whose
    # SEGDEF class is BC_DATA. DGROUP holds it beside the runtime's own
    # data (BR_DATA, BR_SKYS), and they are different segments: a runtime
    # routine writes through its own buffers, not through the program's
    # variables, unless the program handed it a pointer.
    #
    # Without the distinction every call reads as writing every variable,
    # and nothing in the corpus is promotable. With it, 47 cells are.
    program_data: int | None = None
    # Which fixups each operation's own operands carry, by mir.Op.id. One
    # for one instruction, which is every operation the raise makes -- and
    # several for one it folds an idiom into: absorbing a long divide is
    # four instructions and two of them are relocated. Asked
    # once at the raise, while every operation still stands where BC wrote
    # it, because that is the only moment the bytes can answer it. Keyed by
    # the operation rather than by its address: a pass may move it.
    refs: dict[int, tuple[int, ...]] = field(default_factory=dict)
    # Encoding provenance for FP operations raised from runtime helpers.
    # Kept outside MIR; only the frontend and emitter use the protocol.
    float_protocols: dict[int, int] = field(default_factory=dict)
    # Which absorbable call each folded operation stands for, by mir.Op.id.
    # The raise turns a push run and its call into one operation over the
    # argument values; lowering asks this what instructions to write for it.
    # calls.CallSite, but module.py sits below calls.py and cannot say so.
    absorbed: dict[int, object] = field(default_factory=dict)
    # The full, ordered set of disjoint byte ranges a folded operation
    # stands for, by mir.Op.id -- present only for the rare op whose bytes
    # are not one run. A site frames() found may push its arguments, let
    # BC put a real instruction between the pushes and the call, and only
    # then call: one interval cannot name both the pushes and the call
    # without also claiming the instruction between them, so this is the
    # one record of every range instead. Absent, `covers` alone is still
    # the whole answer -- the ordinary case, and every op has it.
    # Established at the raise, beside `absorbed` and `refs`, and asked
    # only where a caller is accounting for every byte: lower.py and every
    # machine phase read `covers` alone and never know this exists.
    coverage: dict[int, tuple[tuple[int, int], ...]] = field(default_factory=dict)

    def resolve(self, field_offset: int, literal: int) -> Addr:
        """What the operand whose displacement field sits here points at."""
        return self.operands.get(field_offset, Addr(Space.LITERAL, literal))


def frame_relative(literal: int) -> Addr:
    return Addr(Space.FRAME, literal)


def far_pointer(literal: int, base: Register_, segment: Register_) -> Addr:
    return Addr(Space.FAR, literal, base=base, segment=segment)


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


def escaped(found: "Module") -> frozenset[tuple[int, int]]:
    """Every (segment, displacement) this object hands out the address of.

    A runtime routine writes its own data at fixed addresses and never a
    cell in BC_DATA -- tools/runtime_writes.py measures it on the linked
    image. So the only way it reaches a program's variable is a pointer the
    program gave it, and this is where those are given.

    A relocated immediate inside a `push`, a register `mov` immediately
    pushed, or a `lea` is what handing one
    over looks like. `Operation.ADDRESS` is not: BC pushes `offset X`,
    which is an immediate the linker fills in, and testing for `lea` found
    none of the corpus's -- so a whole-body test read every program as
    handing out nothing and granted a guarantee none of them had earned.

    The object it names, not the byte: an escaped address poisons the whole
    landmark object, because a routine handed a descriptor's address writes
    at offsets from it.
    """
    from qbopt.objectfile import omf

    fields = [one for one in omf.fixups(found.records) if one.seg == found.seg]
    if not fields:
        return frozenset()
    out = set()
    values = _numeric_arguments(found)
    for at, end, text in _pushes(found):
        if at in values or (text.startswith("mov") and end in values):
            continue
        for one in fields:
            if at <= one.offset < end:
                out.add((one.index, one.disp))
    return frozenset(out)


def _numeric_arguments(found: "Module") -> frozenset[int]:
    """Complete adjacent push groups consumed by known by-value integer PRINTs."""
    from iced_x86 import Mnemonic
    from qbopt.abi import runtime
    from qbopt.frontend import declen

    pending, values = [], set()
    at = found.start
    while at < found.end:
        insn = declen.decode(found.code, at)
        if insn is None:
            pending = []
            at += 1
            continue
        if insn.insn.mnemonic == Mnemonic.PUSH:
            pending.append(insn)
        else:
            width = runtime.integer_print_argument(found.calls.get(at, ""))
            if width is not None and pending and sum(-one.insn.stack_pointer_increment for one in pending) == width:
                values.update(one.at for one in pending)
            pending = []
        at = insn.end
    return frozenset(values)


def _pushes(found: "Module"):
    """Every instruction that could hand an address over, as (at, end, text)."""
    from iced_x86 import Mnemonic, OpKind
    from qbopt.frontend import declen

    at = found.start
    while at < found.end:
        insn = declen.decode(found.code, at)
        if insn is None:
            at += 1
            continue
        text = str(insn.insn).lower()
        # A `mov [x],ax` also carries a relocated
        # field, but that is the store's own displacement -- the address of
        # the cell being written, not an address being handed to anybody.
        # Including it marked every written cell as escaped, which is every
        # cell, and the guarantee came to nothing.
        materialized = (insn.insn.mnemonic == Mnemonic.MOV
                        and insn.insn.op0_kind == OpKind.REGISTER
                        and insn.insn.op1_kind in (OpKind.IMMEDIATE16, OpKind.IMMEDIATE32))
        if materialized:
            following = declen.decode(found.code, insn.end) if insn.end < found.end else None
            materialized = (following is not None and following.insn.mnemonic == Mnemonic.PUSH
                            and following.insn.op0_kind == OpKind.REGISTER
                            and following.insn.op0_register == insn.insn.op0_register)
        if text.startswith(("push", "lea")) or materialized:
            yield at, insn.end, text
        at = insn.end


def landmarks(found: "Module") -> dict[tuple[Space, int], tuple[int, ...]]:
    """Every displacement in each segment that some operand names exactly.

    An indexed operand reads or writes its whole segment as far as
    may_alias is concerned, and that is what stops LICM dead on any loop
    that writes an array -- matrix's inner loop has nothing invariant in it
    because `m(r * w + c)` is taken to reach `w`, `r` and `c`.

    What bounds it is the object's own layout. BC allocates each variable a
    displacement and every non-indexed operand names one exactly, so the
    array beginning at 0x6 cannot run past the next thing named after it,
    which in matrix is 0x328 -- 802 bytes, and `DIM m(400)` to the byte.

    The assumption is that a subscript is in range. Out of range is not
    defined in QuickBASIC without bounds checking, and every optimising
    compiler makes exactly this assumption; it is stated here rather than
    buried, because it is the one thing in this module that is not
    arithmetic on the object.
    """
    found_at: dict[tuple[Space, int], set[int]] = {}
    for addr in found.operands.values():
        if addr.base == Register.NONE and addr.space in (Space.SEGMENT, Space.FRAME):
            found_at.setdefault((addr.space, addr.index), set()).add(addr.disp)
    return {where: tuple(sorted(disps)) for where, disps in found_at.items()}


def reach(addr: Addr, width: int, bounds: dict[tuple[Space, int], tuple[int, ...]]) -> tuple[int, int] | None:
    """The bytes an operand can touch, or None where nothing bounds it."""
    if addr.base == Register.NONE:
        return addr.disp, addr.disp + width
    known = bounds.get((addr.space, addr.index))
    if not known:
        return None
    after = [one for one in known if one > addr.disp]
    return (addr.disp, after[0]) if after else None


def may_alias(
    a: Addr | None,
    b: Addr | None,
    dgroup: frozenset[int],
    a_width: int = WIDEST,
    b_width: int = WIDEST,
    bounds: dict[tuple[Space, int], tuple[int, ...]] | None = None,
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
    A Space.FAR address is exactly this argument one register up: it always
    carries a base (bx, measured), so it falls into the same catch-all --
    unbounded here because *two* registers would have to still hold what
    they held, es as well as bx, and nothing at this layer can see either.
    That also makes two Space.FAR addresses through different segment
    registers, or a Space.FAR address against anything else, alias: neither
    comparison ever reaches the space-specific cases below, which is the
    point -- proving them disjoint needs the same-segment, same-base
    arithmetic memory.py's aliases() does, sound only under the same
    obligation as the SI/DI case above (every tracked cell whose bx or es an
    instruction writes is dropped first).

    Two bare displacements are disjoint when their own ranges do not meet.
    Within one object a SEGDEF index names one segment, and two distinct
    SEGDEFs are two distinct segments, so a differing index is disjoint
    outright; a matching one is the range test. Same for two frame slots,
    where the ranges are bp-relative -- sound only while bp is invariant
    across the region asking, which is the caller's own obligation to check
    (nothing here can see whether something wrote bp), the way registers.py
    already checks a single register's own liveness.

    A frame or stack slot is never the same byte as an addressed variable.
    SS==DS in this model and the stack lives in DGROUP, so the two *could*
    coincide and the object cannot prove they do not -- SS itself does not
    exist until the runtime sets it up. What rules it out is that the stack
    is the last thing in DGROUP and grows down, so it reaches a named
    variable only by overflowing into it, which is a program that has
    already lost. Every optimising compiler assumes locals and globals are
    disjoint on the same argument.

    Refusing to assume it costs the whole of loop-invariant code motion in
    any loop that pushes an argument: one `push` makes every named load in
    the loop alias something, so nothing is invariant. lngmix is two long
    divides over an operand that never changes and it could not move either.

    A stack slot against a frame slot is a different question and stays
    conservative -- both are in the same region and their displacements are
    against different registers.

    This is the one rule here that rests on anything beyond arithmetic, and
    it is centralised rather than re-derived at each call site.

    None stands for "address not known" -- an unresolved operand, or a
    Space.GROUP address (see Space.GROUP's own comment) -- and is never
    provably disjoint from anything.
    """
    if a is None or b is None:
        return True
    if a.base != Register.NONE or b.base != Register.NONE:
        # An indexed operand reaches its whole segment unless a caller has
        # handed over the layout to bound it with. See landmarks().
        if bounds is None or a.space is not b.space or a.index != b.index:
            return True
        here, there = reach(a, a_width, bounds), reach(b, b_width, bounds)
        if here is None or there is None:
            return True
        return here[0] < there[1] and there[0] < here[1]
    match (a.space, b.space):
        case (Space.STACK, Space.STACK):
            return _overlaps(a, a_width, b, b_width)
        case (Space.STACK, Space.FRAME) | (Space.FRAME, Space.STACK):
            return True
        case (Space.STACK, _) | (_, Space.STACK):
            return False
        case (Space.FRAME, Space.FRAME):
            return _overlaps(a, a_width, b, b_width)
        case (Space.SEGMENT, Space.SEGMENT):
            return a.index == b.index and _overlaps(a, a_width, b, b_width)
        case (Space.FRAME, Space.SEGMENT) | (Space.SEGMENT, Space.FRAME):
            return False
        case _:
            return True


# The SEGDEF name BC gives the segment holding a program's own variables.
# The runtime's data is BR_DATA and BR_SKYS, contributed at link time; BC
# emits this one, and everything a compiled program declares lives in it.
PROGRAM_DATA = "BC_DATA"


def _program_data(records: list) -> int | None:
    """Which segment index holds this program's own variables."""
    for index, one in enumerate(omf.segments(records)):
        if one is not None and one[0] == PROGRAM_DATA:
            return index
    return None


def of(records: list[omf.Record]) -> Module | None:
    found = omf.code_segment(records)
    if found is None:
        return None
    seg, name, size = found
    code = omf.segment_image(records, seg, size)
    fixups = [fixup for fixup in omf.fixups(records) if fixup.seg == seg]

    spaces = {"segment": Space.SEGMENT, "group": Space.GROUP, "external": Space.EXTERNAL}
    operands = {
        fixup.offset: Addr(spaces[fixup.target], fixup.disp, fixup.index)
        for fixup in fixups
        if fixup.loc == omf.LOC_OFF16 and fixup.target in spaces
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
    program_data = _program_data(records)

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
        program_data=program_data,
    )


def load(path: Path | str) -> Module | None:
    return of(omf.read(path))
