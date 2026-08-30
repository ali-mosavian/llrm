"""
What has to change when code moves.

The list is finite: the offset a fixup patches and the addend stored there, the
target displacement when it names the segment being moved, public symbol
offsets, line-number tables, the segment's length, the entry point, and
self-relative branches. That last one is the only part with no record to lean
on -- it has to be recomputed by decoding.

An offset landing strictly inside a rewritten region is an error rather than
something to clamp: the ON GOTO table points at arbitrary code offsets, and one
landing inside a region means the region was chosen wrong.
"""

import struct
from dataclasses import field
from dataclasses import dataclass

from qbopt import omf
from qbopt import module
from qbopt.declen import Insn
from qbopt.blocks import code_map
from qbopt.blocks import instructions

# How far in to look for where the header stops and code starts. Measured over
# the corpus: header fields carry fixups up to 0x20, the earliest operand of an
# instruction is at 0x31, and the decodes that satisfy every fixup start at 0x21,
# 0x22 or 0x30. A module needing more is refused rather than guessed at.
HEADER_SEARCH = 0x40

REL8 = frozenset(range(0x70, 0x80)) | {0xEB} | frozenset(range(0xE0, 0xE4))
REL16 = {0xE8, 0xE9} | frozenset(range(0x0F80, 0x0F90))


class Inside(Exception):
    """An offset landed inside a region that was rewritten."""


@dataclass(frozen=True, slots=True, order=True)
class Edit:
    lo: int
    hi: int
    data: bytes
    # (offset within `data`, the fixup to put there). Each is one BC already
    # wrote for the same operand, moved: it names the right target already.
    fixups: tuple[tuple[int, omf.Fixup], ...] = ()

    @property
    def length(self) -> int:
        return len(self.data)

    @property
    def delta(self) -> int:
        return self.length - (self.hi - self.lo)


@dataclass(frozen=True, slots=True)
class Shift:
    edits: tuple[Edit, ...] = field(default_factory=tuple)

    @staticmethod
    def of(edits: list[Edit]) -> "Shift":
        ordered = tuple(sorted(edits))
        for earlier, later in zip(ordered, ordered[1:], strict=False):
            if earlier.hi > later.lo:
                raise ValueError(f"edits overlap: {earlier} and {later}")
        return Shift(ordered)

    def at(self, offset: int) -> int:
        """Where an old offset ends up."""
        delta = 0
        for edit in self.edits:
            if offset >= edit.hi:
                delta += edit.delta
            elif offset > edit.lo:
                raise Inside(f"offset {offset:#x} is inside the region {edit.lo:#x}..{edit.hi:#x}")
            else:
                break
        return offset + delta

    def before(self, offset: int) -> int:
        """Where an old offset ends up, an insertion sitting exactly there left in front of it.

        Only differs from at() for a zero-width edit whose own lo==hi==offset: at()
        is right-biased, correct for a *target* landing on an insertion's own point
        (whatever used to be there is now one byte further out, per AGENTS.md's "LINK
        adds whatever is in the code to the fixup's target"). A branch's own *end* is
        not a target -- the CPU measures its displacement from the first byte
        physically after it in the new image, and for an insertion sitting exactly
        there, that byte is the inserted one itself, not what at()'s delta would place
        in front of it. A non-zero-width edit needs no such distinction: its own lo can
        never be a branch's end without being inside the branch, which at() already
        refuses as Inside.
        """
        delta = 0
        for edit in self.edits:
            if edit.lo == edit.hi == offset:
                break
            if offset >= edit.hi:
                delta += edit.delta
            elif offset > edit.lo:
                raise Inside(f"offset {offset:#x} is inside the region {edit.lo:#x}..{edit.hi:#x}")
            else:
                break
        return offset + delta

    @property
    def grows(self) -> bool:
        return any(edit.delta > 0 for edit in self.edits)


@dataclass(frozen=True, slots=True)
class Branch:
    at: int
    end: int
    field_at: int
    width: int
    target: int


def branches(code: bytes, instructions: list[Insn]) -> list[Branch]:
    """Every self-relative branch among the instructions given."""
    found = []
    for insn in instructions:
        if insn.target is None or insn.imm_at is None:
            continue
        found.append(Branch(insn.at, insn.end, insn.imm_at, insn.imm_len, insn.target))
    return found


def reaches(branch: Branch, displacement: int) -> bool:
    bits = branch.width * 8
    return -(1 << (bits - 1)) <= displacement < 1 << (bits - 1)


def retarget(branch: Branch, shift: Shift) -> int:
    """The displacement this branch needs once everything has moved.

    Mapped from `end`, the branch's own next-instruction address, not from `at`:
    using the instruction's start is off by its length wherever an edit sits
    between the two.
    """
    return shift.at(branch.target) - shift.before(branch.end)


def apply(image: bytes, shift: Shift) -> bytes:
    """The segment with every edit spliced in."""
    out = bytearray()
    at = 0
    for edit in shift.edits:
        out += image[at : edit.lo] + edit.data
        at = edit.hi
    return bytes(out + image[at:])


def crossed_pair(
    chunks: tuple[tuple[int, int], ...], at: int, end: int
) -> tuple[tuple[int, int], tuple[int, int]] | None:
    """The two adjacent LEDATA chunks [at, end) spans, if it crosses exactly one boundary safely.

    None both when nothing needs crossing (the region sits inside one chunk)
    and when crossing would not be safe: a third chunk, or a neighbour that is
    not exactly adjacent in file order and byte position. BC's own backpatch
    records are exactly that -- a later record at an earlier offset -- and
    that shape is refused rather than guessed at, the same way it always was
    for a region that could not fit in one chunk at all.
    """
    for i, (lo, hi) in enumerate(chunks):
        if not (lo <= at < hi):
            continue
        if end <= hi:
            return None  # wholly inside one chunk; nothing to cross
        if i + 1 >= len(chunks):
            return None
        nxt = chunks[i + 1]
        if nxt[0] != hi or end > nxt[1]:
            return None
        return (lo, hi), nxt
    return None


def retarget_branches(image: bytes, instructions: list[Insn], shift: Shift) -> bytes | str:
    """The image with every self-relative branch pointing where it used to.

    Not fixups, so there is no record to lean on: the displacements have to be
    recomputed by decoding. A rel8 that no longer reaches is refused, never
    truncated -- truncation wraps mod 256 and lands mid-instruction, with no
    trap and no diagnostic from LINK.
    """
    found = branches(image, instructions)
    out = bytearray(apply(image, shift))
    for branch in found:
        try:
            moved = retarget(branch, shift)
            field_at = shift.at(branch.field_at)
        except Inside:
            continue  # the branch itself was replaced
        if not reaches(branch, moved):
            return f"a rel{branch.width * 8} at {branch.at:#x} would no longer reach its target"
        out[field_at : field_at + branch.width] = moved.to_bytes(branch.width, "little", signed=True)
    return bytes(out)


def _boundary_overrides(
    records: list[omf.Record], seg: int, edits: tuple[Edit, ...], chunks: tuple[tuple[int, int], ...]
) -> tuple[dict[int, tuple[int, int]], set[int]] | str:
    """Per code-LEDATA record id, the span it actually emits, and which to drop.

    A record not named here emits its own natural span, exactly as before. For
    an edit that crosses one boundary, the leader's span grows to the edit's
    own end and the follower's shrinks to start there -- neither record is
    removed, so no FIXUPP is re-parented to a data LEDATA that never followed
    it in the file, which is what removing one would do. A follower an edit
    swallows whole is dropped: every fixup that would have followed it falls
    inside the edit and is already refused a home below, so nothing is lost.
    """
    entries = {(off, off + len(payload)): r for r, index, off, payload in omf.ledata(records) if index == seg}
    # a chunk with a crossing on each side gets touched twice -- once as a
    # follower (its start moves right) and once as a leader (its end moves
    # right too) -- and both have to compose into one span, not overwrite
    # each other, or whichever edit is processed second silently undoes the
    # first's own shrink.
    starts: dict[int, int] = {}
    ends: dict[int, int] = {}
    dropped: set[int] = set()
    boundaries: set[int] = set()
    for edit in edits:
        if any(lo <= edit.lo and edit.hi <= hi for lo, hi in chunks):
            continue  # wholly inside one chunk; nothing to move
        pair = crossed_pair(chunks, edit.lo, edit.hi)
        if pair is None:
            return f"a region at {edit.lo:#x} crosses a LEDATA boundary"
        leader_span, follower_span = pair
        boundary = leader_span[1]  # == follower_span[0]
        if boundary in boundaries:
            return f"a region at {edit.lo:#x} crosses a LEDATA boundary two edits both reach"
        boundaries.add(boundary)
        leader, follower = entries[leader_span], entries[follower_span]
        if id(leader) in dropped or id(follower) in dropped:
            return f"a region at {edit.lo:#x} crosses a LEDATA boundary a swallowed record also reaches"
        ends[id(leader)] = edit.hi
        if edit.hi < follower_span[1]:
            starts[id(follower)] = edit.hi
        else:
            dropped.add(id(follower))
    overrides = {
        id(record): (starts.get(id(record), off), ends.get(id(record), end))
        for (off, end), record in entries.items()
        if id(record) in starts or id(record) in ends
    }
    return overrides, dropped


def relocate(records: list[omf.Record], seg: int, image: bytes, shift: Shift) -> list[omf.Record] | str:
    """Every record, with the code segment moved and everything naming it updated.

    Each code LEDATA is rewritten where it stands rather than the stream being
    rebuilt: BC interleaves data LEDATA and EXTDEF among the code ones, and
    keeping the order keeps every fixup with the data record it is relative to.
    """
    found = module.of(records)
    if found is None:
        return "the module has no code segment"
    mapped = code_map(found)
    if isinstance(mapped, str):
        return mapped
    reached = instructions(found)
    assert not isinstance(reached, str)

    boundaries = mapped.starts | {len(image)}
    for edit in shift.edits:
        if edit.lo not in boundaries or edit.hi not in boundaries:
            return f"the region {edit.lo:#x}..{edit.hi:#x} does not begin and end on an instruction"
        if any(at not in mapped.starts for at in range(edit.lo, edit.hi) if at in boundaries) or any(
            lo < edit.hi and edit.lo < hi for lo, hi in mapped.tables
        ):
            return f"the region {edit.lo:#x}..{edit.hi:#x} covers something that is not an instruction"

    moved = retarget_branches(image, reached, shift)
    if isinstance(moved, str):
        return moved

    overridden = _boundary_overrides(records, seg, shift.edits, found.chunks)
    if isinstance(overridden, str):
        return overridden
    overrides, dropped = overridden

    owner = omf.last_writers(records, seg, len(image))
    fixups = omf.fixups(records)
    by_record: dict[int, list[omf.Fixup]] = {}
    for fixup in fixups:
        by_record.setdefault(id(fixup.record), []).append(fixup)

    out: list[omf.Record] = []
    covered: tuple[int, int] | None = None  # the new range of the code LEDATA just emitted
    for record in records:
        kind = record.type & 0xFE

        if kind == omf.LEDATA:
            index, at = omf._index(record.body, 0)
            offset = struct.unpack_from("<H", record.body, at)[0]
            payload = record.body[at + 2 :]
            if index != seg:
                out.append(record)
                covered = None
                continue
            if id(record) in dropped:
                covered = None
                continue
            start, end = overrides.get(id(record), (offset, offset + len(payload)))
            lo, hi = shift.at(start), shift.at(end)
            if hi - lo > 1024:
                return f"a region at {start:#x} needs {hi - lo} bytes, more than one LEDATA holds"
            # only the bytes this record finally owns, and that were ever its
            # own payload, come from the new image; the rest are its own, so a
            # no-op rebuild is byte for byte the input. A boundary an edit
            # moved annexes bytes that were never this record's -- there is
            # nothing of its own to restore there, only the edit's new code.
            mine_only = bytearray(moved[lo:hi])
            for at in range(max(start, offset), min(end, offset + len(payload))):
                inside_edit = any(edit.lo < at < edit.hi for edit in shift.edits)
                if owner.get(at) != id(record) and not inside_edit:
                    mine_only[shift.at(at) - lo] = payload[at - offset]
            out.append(omf.ledata_record(seg, lo, bytes(mine_only)))
            covered = (lo, hi)
            continue

        if kind == omf.FIXUPP and (mine := by_record.get(id(record))):
            rebuilt = []
            for fixup in mine:
                if fixup.seg == seg and any(edit.lo <= fixup.offset < edit.hi for edit in shift.edits):
                    continue  # the instruction it patched has been replaced
                into_code = fixup.target == "segment" and fixup.index == seg and fixup.disp_pos is not None
                disp = shift.at(fixup.disp) if into_code else None
                if fixup.seg != seg:
                    rebuilt.append(omf.reemit(fixup, disp=disp))
                    continue
                if covered is None:
                    return "a code fixup does not follow a code LEDATA"
                rebuilt.append(omf.reemit(fixup, offset=shift.at(fixup.offset) - covered[0], disp=disp))
            if covered is not None:
                for edit in shift.edits:
                    placed = shift.at(edit.lo)
                    if not covered[0] <= placed < covered[1]:
                        continue
                    for at, fixup in edit.fixups:
                        rebuilt.append(omf.reemit(fixup, offset=placed + at - covered[0]))
            threads = record.body[: mine[0].lo]
            rebuilt_body = threads + b"".join(rebuilt)
            out.append(record if rebuilt_body == record.body else omf.fixupp_record([rebuilt_body]))
            continue

        if kind == omf.MODEND and omf.has_start_address(record):
            return "MODEND carries a start address, which is not relocated yet"

        out.append(
            omf.patched(
                record,
                {at: shift.at(struct.unpack_from("<H", record.body, at)[0]) for at in omf.code_offsets(record, seg)},
            )
        )

    return _resized(out, seg, len(moved))


def _resized(records: list[omf.Record], seg: int, length: int) -> list[omf.Record]:
    """The SEGDEF for `seg` carrying the segment's new length."""
    out, index = [], 0
    for record in records:
        if record.type & 0xFE != omf.SEGDEF:
            out.append(record)
            continue
        index += 1
        if index != seg:
            out.append(record)
            continue
        at = omf.segment_length_at(record)
        out.append(
            record if struct.unpack_from("<H", record.body, at)[0] == length else omf.patched(record, {at: length})
        )
    return out
