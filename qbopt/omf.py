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


@dataclass(slots=True)
class Fixup:
    """One relocation, resolved to where in the segment it patches."""

    seg: int | None
    offset: int
    loc: int
    selfrel: bool
    target: str
    index: int
    raw: bytes | None

    def __repr__(self) -> str:
        loc = LOCNAME.get(self.loc, self.loc)
        return f"<{self.seg} {self.offset:04X} {loc} {self.target} {self.index}>"


def fixups(recs: list[Record]) -> list[Fixup]:
    """Every FIXUP subrecord, with its offset made absolute in the segment.

    A FIXUPP's offsets are relative to the LEDATA it follows, which is why
    this walks the records in order rather than gathering them by type.

    THREAD subrecords set a default frame or target that later fixups refer
    to by number, and BC leans on them heavily -- 34 of the 40 fixups in a
    module with an ON GOTO and a SELECT CASE were thread-based. Anything
    that means to move code has to resolve them, or it cannot see what most
    of the relocations point at.
    """
    out, seg, base = [], None, 0
    ftr = [None] * 4  # frame threads
    ttr = [None] * 4  # target threads
    for r in recs:
        if r.type & 0xFE == LEDATA:
            si, i = _index(r.body, 0)
            seg, base = si, struct.unpack_from("<H", r.body, i)[0]
            continue
        if r.type & 0xFE != FIXUPP:
            continue
        b_, i = r.body, 0
        while i < len(b_):
            if not (b_[i] & 0x80):  # THREAD
                d = b_[i]
                i += 1
                method, thred = (d >> 2) & 7, d & 3
                idx = 0
                if (method & 3) < 3:  # SEGDEF/GRPDEF/EXTDEF
                    idx, i = _index(b_, i)
                (ftr if (d & 0x40) else ttr)[thred] = (method, idx)
                continue

            loc = (b_[i] >> 2) & 0x0F
            selfrel = not (b_[i] & 0x40)
            off = ((b_[i] & 0x03) << 8) | b_[i + 1]
            i += 2
            fd = b_[i]
            i += 1

            if fd & 0x80:  # frame from a thread
                pass
            elif ((fd >> 4) & 7) < 3:  # explicit frame index
                _, i = _index(b_, i)

            if fd & 0x08:  # target from a thread
                th = ttr[fd & 3]
                method, index = th if th else (7, 0)
            else:
                method = fd & 3
                index, i = _index(b_, i)
            target = TARGET_KIND.get(method & 3, "frame")

            if not (fd & 0x04):  # a displacement follows
                i += 2
            out.append(Fixup(seg, base + off, loc, selfrel, target, index, None))
    return out


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
