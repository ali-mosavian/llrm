"""QuickBASIC Quick libraries (.QLB): read, check, write, dump. The format is docs/formats/qlb.md.

    python3 tools/qlb.py FILE...            one line per file
    python3 tools/qlb.py --symbols FILE     the code and data symbols, then the segments
    python3 tools/qlb.py --check FILE...    the invariants of the format, and emit(parse(file)) == file

`parse(data)` returns a `Qlb` or raises `QlbError`; `Qlb.emit()` returns the bytes. Standard library only.
"""
from __future__ import annotations

import struct
import sys
from dataclasses import dataclass

MAGIC = 0x6C75  # "ul"
VERSION = 2
HEADER = 0x22  # bytes of the SYMBOL block before its first table
ENTRY = 8
FAR_JMP = 0xEA
CODE, NEAR, SEGMENT, COMMON, ABSOLUTE = 1, 2, 3, 4, 5  # table entry flags
COMMON_CLASS = "BC_VARS"  # the class of a segment entered as COMMON
MZ = struct.Struct("<2s13H")  # signature, last page, pages, relocs, header paragraphs, min, max, ss, sp, sum, ip, cs, table, overlay
RELOCS_AT = 0x1E  # after one more word, always 1
PAGE = 512


class QlbError(ValueError):
    pass


@dataclass
class Symbol:
    """A table entry: code, near or segment (offset, frame); absolute (value, 0)."""

    flags: int
    name: str
    offset: int
    frame: int


@dataclass
class Qlb:
    image: bytes  # the image below the SYMBOL block with the left-out zeros put back; a multiple of 16 bytes
    relocs: list[tuple[int, int]]  # (offset, frame) of each word of the image or SYMBOL block that holds a frame
    code: list[Symbol]
    data: list[Symbol]
    segments: list[Symbol]
    jump: tuple[int, int]  # offset, frame of `__aulstart`: the header's JMP FAR
    base: int = 0  # header +0x0E, a frame; always 0
    dgroup: int = 0  # frame of DGROUP; 0 without one
    far_data: tuple[int, int] = (0, 0)  # frame, bytes of the initialised FAR_DATA
    far_bss: tuple[int, int] = (0, 0)  # frame, bytes of the FAR_BSS
    checksum: int = 0  # the MZ checksum, as LINK wrote it
    stack: bool = True  # a STACK segment (at the SYMBOL frame) exists: MZ ss is that frame, else 0
    zeros: tuple[int, int] = (0, 0)  # zero bytes left out of the file at the image start and at DGROUP

    @property
    def symbol_frame(self) -> int:
        return len(self.image) // 16

    def symbol_block(self) -> bytes:
        tables = (self.code, self.data, self.segments)
        starts, at = [], HEADER
        for table in tables:
            starts.append(at)
            at += ENTRY * len(table) + 2
        entries, names = bytearray(), bytearray()
        for table in tables:
            for sym in table:
                entries += struct.pack("<HHHH", sym.flags, at + len(names), sym.offset, sym.frame)
                names += sym.name.encode("latin-1") + b"\0"
            entries += b"\0\0"
        names += b"\0\0"
        size = at + len(names)
        head = struct.pack("<BHHBH3H", FAR_JMP, *self.jump, VERSION, MAGIC, *starts)
        head += struct.pack("<5H", self.base, self.dgroup, self.symbol_frame, size, self.far_data[0])
        head += struct.pack("<HH", self.far_data[1] & 0xFFFF, self.far_data[1] >> 16)
        head += struct.pack("<HHH", self.far_bss[0], self.far_bss[1] & 0xFFFF, self.far_bss[1] >> 16)
        assert len(head) == HEADER
        return head + bytes(entries) + bytes(names)

    def emit(self) -> bytes:
        w0, w1 = self.zeros
        dg = self.dgroup * 16
        if self.dgroup:
            stored = struct.pack("<H", w0) + self.image[w0:dg] + struct.pack("<H", w1) + self.image[dg + w1 :]
        else:
            stored = struct.pack("<H", w0) + self.image[w0:]
        symbol = self.symbol_block()
        header = (RELOCS_AT + 4 * len(self.relocs) + PAGE - 1) // PAGE * PAGE
        size = header + len(self.image) + len(symbol)
        mz = MZ.pack(b"MZ", size % PAGE, (size + PAGE - 1) // PAGE, len(self.relocs), header // 16, 0xFFFF, 0xFFFF,
                     self.symbol_frame if self.stack else 0, 0, self.checksum, len(stored) & 15, len(stored) >> 4, RELOCS_AT, 0)
        table = struct.pack("<H", 1) + b"".join(struct.pack("<HH", off, frame) for off, frame in self.relocs)
        return (mz + table).ljust(header, b"\0") + stored + symbol

    def routines(self) -> list[str]:
        """The code symbols a BASIC program can CALL: no leading `_` (the C runtime), `$`, nor `b_` (the interface)."""
        return [s.name for s in self.code if not s.name.startswith(("_", "$", "b_"))]


def parse(data: bytes) -> Qlb:
    if len(data) < MZ.size + 2 or data[:2] != b"MZ":
        raise QlbError("not an MZ file")
    _, _, _, nrel, hdrpar, lo, hi, ss, sp, csum, ip, cs, table, _ = MZ.unpack_from(data)
    if table != RELOCS_AT or struct.unpack_from("<H", data, MZ.size)[0] != 1:
        raise QlbError("relocation table not at 0x1E after a word of 1")
    if (lo, hi, sp) != (0xFFFF, 0xFFFF, 0):
        raise QlbError("MZ allocation or stack pointer not as LINK /Q writes them")
    first = hdrpar * 16
    q = first + cs * 16 + ip  # the entry point is the file offset of the SYMBOL block, from the image start
    if q + HEADER > len(data):
        raise QlbError("entry point past the end of the file")
    jmp, joff, jseg, version, magic = struct.unpack_from("<BHHBH", data, q)
    if jmp != FAR_JMP or magic != MAGIC:
        raise QlbError("no Quick library header at the entry point")
    if version != VERSION:
        raise QlbError(f"header version {version}; only {VERSION} is known")
    sy, da, sg, base, dgroup, symframe, size, fdf, fdlo, fdhi, fbf, fblo, fbhi = struct.unpack_from("<13H", data, q + 8)
    if ss not in (symframe, 0) or size != len(data) - q or sy != HEADER:
        raise QlbError("SYMBOL frame or size disagrees with the MZ header")
    relocs = [struct.unpack_from("<HH", data, table + 4 * i) for i in range(nrel)]
    block = data[q:]

    def entries(at: int) -> tuple[list[tuple[int, int, int, int]], int]:
        out = []
        while struct.unpack_from("<H", block, at)[0] != 0:
            out.append(struct.unpack_from("<HHHH", block, at))
            at += ENTRY
        return out, at + 2

    code, end = entries(sy)
    if end != da:
        raise QlbError("data table does not follow the code table")
    near, end = entries(da)
    if end != sg:
        raise QlbError("segment table does not follow the data table")
    segs, end = entries(sg)
    at = end
    tables = []
    for raw in (code, near, segs):
        syms = []
        for flags, name_at, offset, frame in raw:
            if name_at != at:
                raise QlbError("names are not laid out in table order")
            stop = block.index(b"\0", at)
            syms.append(Symbol(flags, block[at:stop].decode("latin-1"), offset, frame))
            at = stop + 1
        tables.append(syms)
    if block[at:] != b"\0\0":
        raise QlbError("bytes after the last name")
    stored = data[first:q]
    dg = dgroup * 16
    w0 = struct.unpack_from("<H", stored)[0]
    if dgroup:
        w1 = struct.unpack_from("<H", stored, 2 + dg - w0)[0]
        image = bytes(w0) + stored[2 : 2 + dg - w0] + bytes(w1) + stored[4 + dg - w0 :]
    else:
        w1, image = 0, bytes(w0) + stored[2:]
    if len(image) > symframe * 16:
        raise QlbError("image runs into the SYMBOL block")
    image += bytes(symframe * 16 - len(image))
    return Qlb(image, relocs, *tables, (joff, jseg), base, dgroup, (fdf, fdlo | fdhi << 16), (fbf, fblo | fbhi << 16),
               checksum=csum, zeros=(w0, w1), stack=ss == symframe)


def check(data: bytes) -> list[str]:
    """What is wrong with `data` as a Quick library; empty if nothing is."""
    try:
        lib = parse(data)
    except (QlbError, struct.error, ValueError) as err:
        return [str(err)]
    bad = []
    if lib.emit() != data:
        bad.append("emit(parse(file)) differs from the file")
    frames = {s.name: s.frame for s in lib.segments}
    if "SYMBOL" not in frames or lib.segments[-1].name != "SYMBOL" or frames["SYMBOL"] != lib.symbol_frame:
        bad.append("last segment is not SYMBOL at the SYMBOL frame")
    for name, tab in (("code", lib.code), ("data", lib.data)):
        for sym in tab:
            if sym.frame > lib.symbol_frame:
                bad.append(f"{name} symbol {sym.name} is past the SYMBOL block")
    for sym in lib.segments:
        if sym.flags not in (SEGMENT, COMMON):
            bad.append(f"segment {sym.name} has flags {sym.flags}")
    image = lib.image + lib.symbol_block()
    for off, frame in lib.relocs:
        at = frame * 16 + off
        if at + 2 > len(image) or struct.unpack_from("<H", image, at)[0] > lib.symbol_frame:
            bad.append(f"relocation {frame:04X}:{off:04X} does not hold a frame")
    return bad


def summary(path: str, lib: Qlb) -> str:
    near = sum(s.flags == NEAR for s in lib.data)
    return (f"{path}: {len(lib.code)} code, {near} near, {len(lib.data) - near} absolute, {len(lib.segments)} segments, "
            f"{len(lib.relocs)} relocations, image {len(lib.image):#x}, SYMBOL {lib.symbol_frame:#x}:0, "
            f"zeros {lib.zeros[0]}/{lib.zeros[1]:#x}")


def listing(lib: Qlb) -> str:
    out = ["code"]
    out += [f"  {s.frame:04X}:{s.offset:04X}  {s.name}" for s in lib.code]
    out += ["data"]
    out += [f"  {s.frame:04X}:{s.offset:04X}  {s.name}" if s.flags == NEAR else f"  ABS {s.offset:04X}  {s.name}"
            for s in lib.data]
    out += ["segments"]
    out += [f"  {s.frame:04X}:{s.offset:04X}  {s.name}{'  (common)' if s.flags == COMMON else ''}" for s in lib.segments]
    return "\n".join(out)


def main(argv: list[str]) -> int:
    symbols = "--symbols" in argv
    checking = "--check" in argv
    files = [a for a in argv if not a.startswith("--")]
    if not files:
        print(__doc__, file=sys.stderr)
        return 2
    status = 0
    for path in files:
        data = open(path, "rb").read()
        if checking:
            bad = check(data)
            print(f"{path}: {'ok' if not bad else '; '.join(bad[:5])}")
            status |= bool(bad)
            continue
        try:
            lib = parse(data)
        except QlbError as err:
            print(f"{path}: {err}", file=sys.stderr)
            status = 1
            continue
        print(listing(lib) if symbols else summary(path, lib))
    return status


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
