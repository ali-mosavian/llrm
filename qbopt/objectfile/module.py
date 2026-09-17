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
    # is refused everywhere it matters: regions answers True (never
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
    # NONE except for an address whose selector differs from the default:
    # Space.FAR names its dynamic segment register, and a backend-created
    # Space.LITERAL can name SS for an indirect frame-derived pointer.
    # The selector is part of the machine address's identity: `es:[bx]`,
    # `ss:[bx]` and `ds:[bx]` are not the same byte merely because bx agrees.
    segment: Register_ = Register.NONE

    @property
    def direct(self) -> bool:
        """Whether the address names bytes without a run-time index.

        The machine representation uses ``Register.NONE`` for this, but
        MIR analyses need only the semantic fact.  Keeping the translation
        here prevents passes from naming a physical-register sentinel.
        """
        return self.base == Register.NONE

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


class Group(frozenset):
    """DGROUP's segment indexes, and which of them the link overlays.

    A frozenset, so every `index in dgroup` means what it did. `shared` is
    the COMMON-combined ones: the link lays every object's copy over the
    same bytes, so a name another object defines can be in one. The rest
    of this object's segment bytes are its alone.
    """

    shared: frozenset[int]

    def __new__(cls, members=(), shared=frozenset()):
        self = super().__new__(cls, members)
        self.shared = frozenset(shared)
        return self


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
    # regions rests on.
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

    A relocated immediate inside a `push`, a register `mov` subsequently
    pushed without being overwritten, or a `lea` is what handing one
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
    instructions = _instructions(found)
    if instructions is None:
        return frozenset((one.index, one.disp) for one in fields)
    out = set()
    values = _numeric_arguments(found, instructions)
    for at, end, pushed in _pushes(found, instructions):
        if at in values or pushed in values:
            continue
        for one in fields:
            if at <= one.offset < end:
                out.add((one.index, one.disp))
    return frozenset(out)


def _instructions(found: "Module") -> list | None:
    """The code map's instructions in address order, or None where there is no map.

    Not a sweep from the segment's start: that decodes the module header as
    code, and on /G3 FPCALC it was still out of step at `mov bx,offset
    inputValue`, so READ's destination never escaped.
    """
    from qbopt.frontend import blocks
    from qbopt.frontend import declen

    mapped = blocks.code_map(found)
    if isinstance(mapped, str):
        return None
    decoded = (declen.decode(found.code, at) for at in sorted(mapped.starts))
    return [insn for insn in decoded if insn is not None]


def _numeric_arguments(found: "Module", instructions: list | None = None) -> frozenset[int]:
    """Numeric argument pushes, including nested long-arithmetic call frames."""
    from iced_x86 import Mnemonic

    from qbopt.abi import runtime

    local = defines(found.records, found.seg)
    calls = {at: name for at, name in found.calls.items() if name not in local}
    pending, values, end = [], set(), None
    for insn in _instructions(found) or () if instructions is None else instructions:
        at = insn.at
        if at != end:
            pending = []
        end = insn.end
        if insn.insn.mnemonic == Mnemonic.PUSH:
            pending.append(insn)
        else:
            width = runtime.numeric_stack_arguments(calls.get(at, ""))
            if width is not None:
                consumed, total = [], 0
                for one in reversed(pending):
                    total -= one.insn.stack_pointer_increment
                    consumed.append(one.at)
                    if total >= width:
                        if total == width:
                            values.update(consumed)
                        break
            pending = []
    from qbopt.frontend import stack
    from qbopt.frontend import blocks

    def long_arity(name):
        return 2 if runtime.numeric_stack_arguments(name) == 8 else None

    if any(long_arity(name) is not None for name in calls.values()):
        for block in blocks.partition(found, blocks.code_map(found)):
            for frame in stack.frames(block, calls, long_arity):
                values.update(one.at for one in frame.pushed)
    return frozenset(values)


def _pushes(found: "Module", instructions: list | None = None):
    """Address-bearing spans and the push consuming a materialized address."""
    from iced_x86 import OpKind
    from iced_x86 import Mnemonic

    for insn in _instructions(found) or () if instructions is None else instructions:
        at = insn.at
        text = str(insn.insn).lower()
        # A `mov [x],ax` also carries a relocated
        # field, but that is the store's own displacement -- the address of
        # the cell being written, not an address being handed to anybody.
        # Including it marked every written cell as escaped, which is every
        # cell, and the guarantee came to nothing.
        materialized = (
            insn.insn.mnemonic == Mnemonic.MOV
            and insn.insn.op0_kind == OpKind.REGISTER
            and insn.insn.op1_kind in (OpKind.IMMEDIATE16, OpKind.IMMEDIATE32)
        )
        pushed = _pushed_before_write(found, insn.end, insn.insn.op0_register) if materialized else None
        materialized = pushed is not None
        if text.startswith(("push", "lea")) or materialized:
            yield at, insn.end, pushed


def _pushed_before_write(found: "Module", at: int, register: Register_) -> int | None:
    from iced_x86 import OpKind
    from iced_x86 import Mnemonic
    from iced_x86 import FlowControl
    from iced_x86 import RegisterExt

    from qbopt.frontend import declen

    root = RegisterExt.full_register32(register)
    while at < found.end:
        one = declen.decode(found.code, at)
        if one is None or one.insn.flow_control != FlowControl.NEXT:
            return None
        if (
            one.insn.mnemonic == Mnemonic.PUSH
            and one.insn.op0_kind == OpKind.REGISTER
            and RegisterExt.full_register32(one.insn.op0_register) == root
        ):
            return at
        if any(
            access.access in declen.WRITES and RegisterExt.full_register32(access.register) == root
            for access in declen.INFO.info(one.insn).used_registers()
        ):
            return None
        at = one.end
    return None


def landmarks(found: "Module") -> dict[tuple[Space, int], tuple[int, ...]]:
    """Every displacement in each segment that some operand names exactly.

    An indexed operand reads or writes its whole segment as far as
    regions is concerned, and that is what stops LICM dead on any loop
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
    shared = frozenset(index for index, kind in omf.combines(records).items() if kind == omf.COMBINE_COMMON)
    dgroup = Group(omf.groups(records).get(DGROUP, ()), shared)
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
