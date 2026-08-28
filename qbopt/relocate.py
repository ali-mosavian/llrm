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
from qbopt.declen import run
from qbopt.declen import Insn
from qbopt.declen import to_signed

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


def branches(code: bytes, start: int, end: int) -> tuple[list[Branch], int | None]:
    """Every self-relative branch, and where decoding gave up."""
    instructions, gave_up = run(code, start, end)
    found = []
    for insn in instructions:
        width = 1 if insn.opcode in REL8 else 2 if insn.opcode in REL16 else 0
        if not width or insn.imm_at is None:
            continue
        raw = int.from_bytes(code[insn.imm_at : insn.imm_at + insn.imm_len], "little")
        found.append(Branch(insn.at, insn.end, insn.imm_at, insn.imm_len, insn.end + to_signed(raw, insn.imm_len)))
    return found, gave_up


def reaches(branch: Branch, displacement: int) -> bool:
    bits = branch.width * 8
    return -(1 << (bits - 1)) <= displacement < 1 << (bits - 1)


def retarget(branch: Branch, shift: Shift) -> int:
    """The displacement this branch needs once everything has moved.

    Mapped from `end`, the branch's own next-instruction address, not from `at`:
    using the instruction's start is off by its length wherever an edit sits
    between the two.
    """
    return shift.at(branch.target) - shift.at(branch.end)


def trusted_decode(image: bytes, patched: list[int]) -> tuple[int, list[Insn]] | str:
    """A linear decode that accounts for every fixup site, and where it starts.

    A module's code segment opens with a header that is data, so decoding from
    zero misaligns and reports boundaries that fall inside immediates -- which is
    how a nop inserted "at a boundary" ends up splitting the constant next to it.
    The fixups are an independent, BC-authored map of where operand fields are,
    so they are the oracle: a decode is trusted only if every site it should
    cover falls inside a displacement or an immediate.

    There may be no such decode. A module with an ON GOTO or a SELECT CASE has
    its jump table in the code segment, and those words are fixup sites that
    belong to no instruction. Nothing linear can explain them, and motion is
    refused until the block builder can say which bytes are code.
    """
    for start in range(HEADER_SEARCH):
        instructions, gave_up = run(image, start, len(image))
        if gave_up is not None:
            continue
        fields = set()
        for insn in instructions:
            if insn.disp_at is not None:
                fields |= set(range(insn.disp_at, insn.disp_at + insn.disp_len))
            if insn.imm_at is not None:
                fields |= set(range(insn.imm_at, insn.imm_at + insn.imm_len))
        if all(site in fields for site in patched if site >= start):
            return start, instructions
    return "no linear decode accounts for every fixup site; the segment holds data"


def apply(image: bytes, shift: Shift) -> bytes:
    """The segment with every edit spliced in."""
    out = bytearray()
    at = 0
    for edit in shift.edits:
        out += image[at : edit.lo] + edit.data
        at = edit.hi
    return bytes(out + image[at:])


def straddled(shift: Shift, lo: int, hi: int) -> Edit | None:
    """An edit that crosses the boundary of the range lo..hi, if there is one."""
    for edit in shift.edits:
        if edit.lo < hi and lo < edit.hi and not (lo <= edit.lo and edit.hi <= hi):
            return edit
    return None


def retarget_branches(image: bytes, start: int, shift: Shift) -> bytes | str:
    """The image with every self-relative branch pointing where it used to.

    Not fixups, so there is no record to lean on: the displacements have to be
    recomputed by decoding. A rel8 that no longer reaches is refused, never
    truncated -- truncation wraps mod 256 and lands mid-instruction, with no
    trap and no diagnostic from LINK.
    """
    found, gave_up = branches(image, start, len(image))
    if gave_up is not None:
        return f"the decoder gave up at {gave_up:#x}, so branches cannot be recomputed"

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


def relocate(records: list[omf.Record], seg: int, image: bytes, shift: Shift) -> list[omf.Record] | str:
    """Every record, with the code segment moved and everything naming it updated.

    Each code LEDATA is rewritten where it stands rather than the stream being
    rebuilt: BC interleaves data LEDATA and EXTDEF among the code ones, and
    keeping the order keeps every fixup with the data record it is relative to.
    """
    sites = [fixup.offset for fixup in omf.fixups(records) if fixup.seg == seg]
    decoded = trusted_decode(image, sorted(sites))
    if isinstance(decoded, str):
        return decoded
    start, instructions = decoded

    boundaries = {insn.at for insn in instructions} | {len(image)}
    for edit in shift.edits:
        if edit.lo not in boundaries or edit.hi not in boundaries:
            return f"the region {edit.lo:#x}..{edit.hi:#x} does not begin and end on an instruction"

    moved = retarget_branches(image, start, shift)
    if isinstance(moved, str):
        return moved

    owner = omf.last_writers(records, seg, len(image))
    fixups = omf.fixups(records)
    by_record: dict[int, list[omf.Fixup]] = {}
    for fixup in fixups:
        by_record.setdefault(id(fixup.record), []).append(fixup)

    out: list[omf.Record] = []
    covered = None  # the new range of the code LEDATA just emitted
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
            if crossing := straddled(shift, offset, offset + len(payload)):
                return f"a region at {crossing.lo:#x} crosses a LEDATA boundary"
            lo, hi = shift.at(offset), shift.at(offset + len(payload))
            # only the bytes this record finally owns come from the new image;
            # the rest are its own, so a no-op rebuild is byte for byte the input
            mine_only = bytearray(moved[lo:hi])
            for at in range(offset, offset + len(payload)):
                if owner.get(at) != id(record):
                    mine_only[shift.at(at) - lo] = payload[at - offset]
            out.append(omf.ledata_record(seg, lo, bytes(mine_only)))
            covered = lo
            continue

        if kind == omf.FIXUPP and (mine := by_record.get(id(record))):
            rebuilt = []
            for fixup in mine:
                into_code = fixup.target == "segment" and fixup.index == seg and fixup.disp_pos is not None
                disp = shift.at(fixup.disp) if into_code else None
                if fixup.seg != seg:
                    rebuilt.append(omf.reemit(fixup, disp=disp))
                    continue
                if covered is None:
                    return "a code fixup does not follow a code LEDATA"
                rebuilt.append(omf.reemit(fixup, offset=shift.at(fixup.offset) - covered, disp=disp))
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
