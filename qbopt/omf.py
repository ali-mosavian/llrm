#!/usr/bin/env python3
"""
Read and write Intel OMF object files -- the format BC hands to LINK.

This is the ground the post-compilation pass stands on. Doing the work here
instead of at run time changes what is knowable rather than only where the
code lives:

  * A call site comes from a FIXUPP naming an EXTDEF, so "is this call
    B$MUI4" is a lookup rather than a segment:offset compare that has to be
    right about relocation.
  * Relocations are records to be edited, not a hazard to be avoided.
  * Nothing ships. The runtime pass costs about 16K of the program it is
    trying to speed up, and every byte of that competes with BC's own 46K.
  * Tests run on the host in milliseconds rather than through DOSBox.

Records are: type byte, then a length word covering the body AND a trailing
checksum byte, so a record occupies 3 + length bytes. Types with bit 0 set
carry 32-bit fields; BC emits the 16-bit forms throughout, and records are
kept as raw bodies so a file that is read and written again is identical
byte for byte. That round trip is the first test: nothing may be rewritten
until nothing is disturbed.

The record envelope, the SEGDEF attribute layout and the FIXUPP encoding
here were read off d32x's linker (src/linker/src/omf.rs), which documents
them precisely and writes objects LINK accepts.

    python3 tools/qbe/omf.py FILE.OBJ            list what is in it
"""

import sys
import struct
from pathlib import Path
from dataclasses import dataclass

THEADR, COMENT, MODEND, EXTDEF = 0x80, 0x88, 0x8A, 0x8C
PUBDEF, LINNUM, LNAMES, SEGDEF = 0x90, 0x94, 0x96, 0x98
GRPDEF, FIXUPP, LEDATA, LIDATA = 0x9A, 0x9C, 0xA0, 0xA2

NAMES = {
    0x80: "THEADR",
    0x88: "COMENT",
    0x8A: "MODEND",
    0x8B: "MODEND32",
    0x8C: "EXTDEF",
    0x90: "PUBDEF",
    0x91: "PUBDEF32",
    0x94: "LINNUM",
    0x95: "LINNUM32",
    0x96: "LNAMES",
    0x98: "SEGDEF",
    0x99: "SEGDEF32",
    0x9A: "GRPDEF",
    0x9C: "FIXUPP",
    0x9D: "FIXUPP32",
    0xA0: "LEDATA",
    0xA1: "LEDATA32",
    0xA2: "LIDATA",
    0xA3: "LIDATA32",
    0xB0: "COMDEF",
    0xB4: "LEXTDEF",
    0xB6: "LPUBDEF",
    0xB8: "LCOMDEF",
    0x8E: "TYPDEF",
}


@dataclass(slots=True)
class Record:
    type: int
    body: bytes

    @property
    def name(self) -> str:
        return NAMES.get(self.type, f"{self.type:02X}")

    def emit(self) -> bytes:
        # the checksum byte makes the record's bytes sum to zero mod 256;
        # a zero byte is also accepted and is what many tools write, but
        # matching what BC wrote keeps a untouched file untouched
        body = self.body
        head = struct.pack("<BH", self.type, len(body) + 1)
        return head + body + bytes([(-sum(head) - sum(body)) & 0xFF])


def read(path: Path | str) -> list[Record]:
    """Every record in the file, in order."""
    return parse(Path(path).read_bytes())


def parse(d: bytes) -> list[Record]:
    out, i = [], 0
    while i + 3 <= len(d):
        t, n = struct.unpack_from("<BH", d, i)
        if i + 3 + n > len(d) + 1:
            raise ValueError(f"record at {i} runs past the end")
        out.append(Record(t, d[i + 3 : i + 2 + n]))  # body, less the checksum
        i += 3 + n
    if i != len(d):
        raise ValueError(f"{len(d) - i} trailing bytes")
    return out


def write(path: Path | str, recs: list[Record]) -> None:
    Path(path).write_bytes(b"".join(r.emit() for r in recs))


def _index(b: bytes, i: int) -> tuple[int, int]:
    """An OMF index: one byte under 128, otherwise two with the top bit set."""
    if b[i] & 0x80:
        return ((b[i] & 0x7F) << 8) | b[i + 1], i + 2
    return b[i], i + 1


def names(recs: list[Record]) -> list[str]:
    """The LNAMES strings, 1-based as every other record refers to them."""
    out = [""]
    for r in recs:
        if r.type & 0xFE == LNAMES:
            i = 0
            while i < len(r.body):
                n = r.body[i]
                out.append(r.body[i + 1 : i + 1 + n].decode("latin1"))
                i += 1 + n
    return out


def segments(recs: list[Record]) -> list[tuple[str, int] | None]:
    """SEGDEFs as (name, length), 1-based by segment index."""
    nm, out = names(recs), [None]
    for r in recs:
        if r.type & 0xFE != SEGDEF:
            continue
        acbp, i = r.body[0], 1
        if (acbp >> 5) == 0:  # absolute: frame and offset follow
            i += 3
        ln = struct.unpack_from("<H", r.body, i)[0]
        if (acbp & 0x02) and ln == 0:  # the big bit: a full 64K
            ln = 0x10000
        i += 2
        ni, i = _index(r.body, i)
        out.append((nm[ni] if ni < len(nm) else "?", ln))
    return out


def externals(recs: list[Record]) -> list[str]:
    """EXTDEF names, 1-based -- FIXUPP targets refer to these by index."""
    out = [""]
    for r in recs:
        if r.type & 0xFE != EXTDEF:
            continue
        i = 0
        while i < len(r.body):
            n = r.body[i]
            out.append(r.body[i + 1 : i + 1 + n].decode("latin1"))
            i += 1 + n
            _, i = _index(r.body, i)  # the type index, unused here
    return out


def ledata(recs: list[Record]) -> list[tuple[Record, int, int, bytes]]:
    """Each LEDATA as (record, segment index, offset, bytes)."""
    out = []
    for r in recs:
        if r.type & 0xFE != LEDATA:
            continue
        si, i = _index(r.body, 0)
        off = struct.unpack_from("<H", r.body, i)[0]
        out.append((r, si, off, r.body[i + 2 :]))
    return out


LOC_LOBYTE, LOC_OFF16, LOC_BASE, LOC_PTR32, LOC_HIBYTE, LOC_OFF32 = 0, 1, 2, 3, 4, 9

TARGET_KIND = {0: "segment", 1: "group", 2: "external"}

LOCNAME = {
    0: "lobyte",
    1: "offset16",
    2: "base",
    3: "ptr16:16",
    4: "hibyte",
    5: "offset16(ldr)",
    9: "offset32",
    11: "ptr16:32",
    13: "offset32(ldr)",
}


@dataclass(frozen=True, slots=True)
class Thread:
    """A frame or target a later fixup refers to by number."""

    method: int
    index: int


@dataclass(slots=True)
class Fixup:
    """One relocation, resolved to where in the segment it patches."""

    seg: int | None
    offset: int
    loc: int
    selfrel: bool
    target: str
    index: int
    # In an object a label's address is not in the code -- the bytes there are
    # zero and this is where the offset lives.
    disp: int
    frame: Thread | int | None
    record: Record
    lo: int
    hi: int
    disp_pos: int | None

    @property
    def raw(self) -> bytes:
        return self.record.body[self.lo : self.hi]

    def __repr__(self) -> str:
        return (
            f"<{self.seg} {self.offset:04X} {LOCNAME.get(self.loc, self.loc)} {self.target} {self.index}+{self.disp}>"
        )


def read_thread(body: bytes, at: int) -> tuple[bool, int, Thread, int]:
    """(is a frame thread, its number, what it names, where the next starts)."""
    lead = body[at]
    at += 1
    method, number = (lead >> 2) & 7, lead & 3
    index = 0
    # Frame methods 4 and 5 -- the location's segment, and the target's frame --
    # carry no index. Testing method & 3 says both of them do, and eats a byte
    # that is not there, which desynchronises the rest of the record.
    if method < 3:
        index, at = _index(body, at)
    return bool(lead & 0x40), number, Thread(method, index), at


def code_segment(records: list[Record]) -> tuple[int, str, int] | None:
    """The module's code segment as (index, name, length), or None."""
    for index, segment in enumerate(segments(records)):
        if segment and segment[0].endswith("_CODE"):
            return index, segment[0], segment[1]
    return None


def segment_image(records: list[Record], seg: int, size: int) -> bytes:
    """The segment's bytes, with every LEDATA applied in file order.

    BC emits overlapping LEDATA: short backpatch records arrive later in the
    file at earlier offsets, filling in forward jump displacements and jump
    table slots. Measured across the fixtures: 18 doubly-covered bytes in
    jumptable.obj, 2 each in three others and 0 in pds-g2.obj -- so an
    assembler checked only against that one looks correct. The last write to a
    byte is the one that counts.
    """
    image = bytearray(size)
    for _record, index, offset, payload in ledata(records):
        if index == seg:
            image[offset : offset + len(payload)] = payload
    return bytes(image)


# The odd-numbered twin of each record type is its 32-bit form. Every decoder
# here matches with & 0xFE, so it accepts them, and then reads them with 16-bit
# struct formats and fixed two-byte skips. BC emits none of them.
WIDE = {MODEND + 1, PUBDEF + 1, LINNUM + 1, SEGDEF + 1, FIXUPP + 1, LEDATA + 1, LIDATA + 1}
COMDAT = {0xC2, 0xC3}


def last_writers(records: list[Record], seg: int, size: int) -> dict[int, int]:
    """Per byte of the segment, which LEDATA record finally wrote it.

    BC's backpatch records mean an earlier record's bytes are not what the
    segment ends up holding at those offsets. A rewriter that hands every record
    the final image would put the patched value in both, which is harmless to
    LINK and destroys byte-identity -- and byte-identity on a no-op is the only
    guard that says the writer disturbs nothing.
    """
    owner = {}
    for record, index, offset, payload in ledata(records):
        if index == seg:
            for at in range(offset, min(offset + len(payload), size)):
                owner[at] = id(record)
    return owner


def refusals(records: list[Record]) -> list[str]:
    """Why this module must be left alone, if it must. Empty means it may be read."""
    reasons = []
    kinds = {record.type for record in records}
    if kinds & {LIDATA, LIDATA + 1}:
        # fixups() tracks its base from LEDATA only, so a FIXUPP after a LIDATA
        # is attributed to the previous LEDATA and comes out at the wrong offset
        reasons.append("LIDATA: fixup offsets after it would be wrong")
    if kinds & COMDAT:
        reasons.append("COMDAT is not decoded")
    if wide := kinds & WIDE:
        reasons.append(f"32-bit records are decoded as 16-bit: {sorted(hex(kind) for kind in wide)}")
    return reasons


def fixups(records: list[Record]) -> list[Fixup]:
    """Every FIXUP subrecord, with its offset made absolute in the segment.

    A FIXUPP's offsets are relative to the LEDATA it follows, which is why this
    walks the records in order rather than gathering them by type.

    THREAD subrecords set a default frame or target that later fixups refer to
    by number, and BC leans on them heavily -- 34 of the 40 fixups in a module
    with an ON GOTO and a SELECT CASE were thread-based. Anything that means to
    move code has to resolve them, or it cannot see what most of the relocations
    point at.
    """
    found: list[Fixup] = []
    seg: int | None = None
    base = 0
    frame_threads: list[Thread | None] = [None] * 4
    target_threads: list[Thread | None] = [None] * 4

    for record in records:
        if record.type & 0xFE == LEDATA:
            segment_index, at = _index(record.body, 0)
            seg, base = segment_index, struct.unpack_from("<H", record.body, at)[0]
            continue
        if record.type & 0xFE != FIXUPP:
            continue

        body, at = record.body, 0
        while at < len(body):
            start = at
            if not body[at] & 0x80:
                is_frame, number, thread, at = read_thread(body, at)
                (frame_threads if is_frame else target_threads)[number] = thread
                continue

            loc = (body[at] >> 2) & 0x0F
            selfrel = not body[at] & 0x40
            offset = ((body[at] & 0x03) << 8) | body[at + 1]
            at += 2
            fixdata = body[at]
            at += 1

            frame: Thread | int | None = None
            if fixdata & 0x80:
                frame = frame_threads[(fixdata >> 4) & 3]
            elif ((fixdata >> 4) & 7) < 3:
                frame, at = _index(body, at)

            if fixdata & 0x08:
                named = target_threads[fixdata & 3]
                method, index = (named.method, named.index) if named else (7, 0)
            else:
                method = fixdata & 3
                index, at = _index(body, at)

            disp, disp_pos = 0, None
            if not fixdata & 0x04:
                disp, disp_pos = struct.unpack_from("<H", body, at)[0], at
                at += 2

            found.append(
                Fixup(
                    seg=seg,
                    offset=base + offset,
                    loc=loc,
                    selfrel=selfrel,
                    target=TARGET_KIND.get(method & 3, "frame"),
                    index=index,
                    disp=disp,
                    frame=frame,
                    record=record,
                    lo=start,
                    hi=at,
                    disp_pos=disp_pos,
                )
            )
    return found


def reemit(fixup: Fixup, offset: int | None = None, disp: int | None = None) -> bytes:
    """This fixup's own bytes, with the fields given replaced. None leaves one alone.

    Only two fixed positions ever change. The thread encoding survives because
    nothing on this path looks at it -- which is the point: expanding threads to
    explicit form would rewrite every fixup's bytes and turn the diff between
    input and output from a short list into the whole FIXUPP section.
    """
    out = bytearray(fixup.raw)
    if offset is not None:
        if not 0 <= offset < 1024:
            raise ValueError(f"a fixup offset is ten bits; {offset:#x} does not fit")
        out[0] = (out[0] & 0xFC) | (offset >> 8)
        out[1] = offset & 0xFF
    if disp is not None:
        if fixup.disp_pos is None:
            raise ValueError("this fixup carries no displacement")
        struct.pack_into("<H", out, fixup.disp_pos - fixup.lo, disp)
    return bytes(out)


def ledata_record(seg: int, offset: int, payload: bytes) -> Record:
    if len(payload) > 1024:
        raise ValueError(f"LEDATA holds at most 1024 bytes, not {len(payload)}")
    return Record(LEDATA, _emit_index(seg) + struct.pack("<H", offset) + payload)


def fixupp_record(subrecords: list[bytes]) -> Record:
    return Record(FIXUPP, b"".join(subrecords))


def _emit_index(value: int) -> bytes:
    return bytes([value]) if value < 0x80 else bytes([0x80 | (value >> 8), value & 0xFF])


def segment_length_at(record: Record) -> int:
    """Where SEGDEF keeps its length. Absolute segments push it three bytes on."""
    return 4 if record.body[0] >> 5 == 0 else 1


def code_offsets(record: Record, seg: int) -> list[int]:
    """Byte positions in `record.body` of 16-bit offsets into segment `seg`.

    This is the list AGENTS.md calls finite, minus the fixups themselves and the
    self-relative branches, which have their own paths.
    """
    body = record.body
    match record.type & 0xFE:
        case t if t == PUBDEF:
            _group, at = _index(body, 0)
            base, at = _index(body, at)
            if base == 0:  # an absolute segment names its frame instead
                at += 2
            if base != seg:
                return []
            found = []
            while at < len(body):
                at += 1 + body[at]  # the name
                found.append(at)
                at += 2  # the offset
                _type, at = _index(body, at)
            return found
        case t if t == LINNUM:
            _group, at = _index(body, 0)
            base, at = _index(body, at)
            if base != seg:
                return []
            return list(range(at + 2, len(body), 4))  # (line, offset) pairs
        case _:
            return []


def patched(record: Record, values: dict[int, int]) -> Record:
    """A copy of `record` with 16-bit fields replaced. Unchanged records are not copied."""
    if not values:
        return record
    body = bytearray(record.body)
    for at, value in values.items():
        struct.pack_into("<H", body, at, value)
    return Record(record.type, bytes(body))


def has_start_address(record: Record) -> bool:
    return bool(record.body[0] & 0x40)


def main(path: Path | str) -> None:
    recs = read(path)
    segs, exts = segments(recs), externals(recs)
    counts = {}
    for r in recs:
        counts[r.name] = counts.get(r.name, 0) + 1
    print(path)
    print("  records:", ", ".join(f"{k} {v}" for k, v in sorted(counts.items())))
    print("  segments:")
    for i, s in enumerate(segs):
        if s:
            print(f"    {i:2d} {s[0]:<16} {s[1]:6d}")
    code = sum(len(b) for _, _, _, b in ledata(recs))
    print(f"  LEDATA bytes: {code}")
    if len(exts) > 1:
        print("  externals: " + ", ".join(exts[1:]))
    fx = fixups(recs)
    print(f"  fixups: {len(fx)}")
    for f in fx:
        if f.target == "external":
            nm = exts[f.index] if f.index < len(exts) else f"?{f.index}"
            loc = LOCNAME.get(f.loc, f.loc)
            print(f"    seg {f.seg} {f.offset:04X}  {loc:<10} {nm}")


if __name__ == "__main__":
    for p in sys.argv[1:]:
        main(p)
