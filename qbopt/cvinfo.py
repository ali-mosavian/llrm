"""
Read BC's own debug symbols straight out of the .OBJ, no LINK or CVPACK.

/Zi makes BC populate two segments it always declares but normally leaves
empty: $$SYMBOLS (one variable-length record per name) and $$TYPES (the type
table those records index into). This is not the CV4 form CVPACK writes into
a linked .EXE (that one prefixes every record with a 2-byte length and a
documented 2-byte S_* type -- see cv4.h in Open Watcom). BC's own pre-link
records are a byte shorter per field throughout: length and kind are each one
byte, and lexical-scope linkage (pParent/pEnd/pNext) is absent because
CVPACK is what fills it in. The kind bytes below were recovered by compiling
the same source through BC, then through LINK /CODEVIEW and CVPACK, and
matching each raw record's fields against its packed CV4 counterpart --
see docs/handover.md-adjacent conversation, not a Microsoft spec, since
this pre-4.0 layout isn't the one documented in TIS's "Microsoft Symbol and
Type Information".

BASIC's CONST is a compile-time substitution with no storage, and BC emits
no symbol record for one at all -- checked empirically, not assumed. There is
therefore no type to print for one either; that isn't a gap in this module.

A scalar's type_index is one of a handful of small, compiler-independent
codes for BASIC's built-in types (INTEGER/LONG/SINGLE/DOUBLE/STRING) --
recovered the same way as the record kinds, by compiling probe programs and
diffing against CVPACK's fully-documented CV4 output. PRIMITIVES below is
that table. Anything else -- an array, a user-defined TYPE, or a procedure's
own return type -- is a module-local index into $$TYPES, and unlike
$$SYMBOLS that segment carries no per-entry length prefix: walking it needs
the full CV4 leaf grammar (LF_POINTER, LF_ARGLIST, LF_PROCEDURE, LF_FIELDLIST,
LF_MEMBER, LF_STRUCTURE, ...), which was decoded from a CVPACK'd .EXE but not
back onto the raw pre-link bytes, so it isn't resolved here. A FUNCTION's
return type is instead read off BASIC's own type-suffix sigil on the
function's name where the source wrote one ($/%/&/!/#) -- reliable because
it's the source's own convention, not a reconstruction of the debug format.
"""

from pathlib import Path
from dataclasses import field
from dataclasses import dataclass
from collections.abc import Iterator

from qbopt import omf

BLOCK, PROC, END, BPREL, LDATA, LABEL = 0x00, 0x01, 0x02, 0x04, 0x05, 0x0B

# type_index -> BASIC scalar type, for the codes that turned up on a DIM or a
# parameter's own record. STRING has two: which one a compiler picks looks
# tied to its near/far string memory model, not to anything in the source.
PRIMITIVES = {
    0x81: "INTEGER",
    0x82: "LONG",
    0x88: "SINGLE",
    0x89: "DOUBLE",
    0x97: "STRING",
    0x9C: "STRING",
}

# FUNCTION's return type, read off its own name -- BASIC's own convention,
# not something reconstructed from $$TYPES.
SIGILS = {"%": "INTEGER", "&": "LONG", "!": "SINGLE", "#": "DOUBLE", "$": "STRING"}


def type_name(type_index: int) -> str | None:
    return PRIMITIVES.get(type_index)


@dataclass(frozen=True, slots=True)
class Local:
    """A procedure's own parameter or local: BP-relative, sign says which."""

    name: str
    bp_offset: int
    type_index: int

    @property
    def is_param(self) -> bool:
        return self.bp_offset > 0

    @property
    def type_name(self) -> str | None:
        return type_name(self.type_index)


@dataclass(frozen=True, slots=True)
class Variable:
    """A module-level DIM: an absolute offset into a data segment."""

    name: str
    offset: int
    segment: int
    type_index: int

    @property
    def type_name(self) -> str | None:
        return type_name(self.type_index)


@dataclass(frozen=True, slots=True)
class Label:
    name: str
    offset: int


@dataclass(frozen=True, slots=True)
class Procedure:
    name: str
    offset: int
    proc_length: int
    debug_start: int
    debug_end: int
    flags: int
    locals: list[Local] = field(default_factory=list)

    @property
    def params(self) -> list[Local]:
        return [loc for loc in self.locals if loc.is_param]

    @property
    def own_locals(self) -> list[Local]:
        return [loc for loc in self.locals if not loc.is_param]

    @property
    def return_type(self) -> str | None:
        """A FUNCTION's return type from its own name's sigil; None for a SUB."""
        return SIGILS.get(self.name[-1]) if self.name else None


@dataclass(frozen=True, slots=True)
class DebugInfo:
    module: str | None
    procedures: list[Procedure]
    variables: list[Variable]
    labels: list[Label]


def _pstr(buf: bytes, at: int) -> tuple[str, int]:
    n = buf[at]
    return buf[at + 1 : at + 1 + n].decode("latin1"), at + 1 + n


def _records(buf: bytes) -> "Iterator[tuple[int, bytes]]":
    """(kind, data) for each record; data excludes the length and kind bytes."""
    at = 0
    while at < len(buf):
        length = buf[at]
        if length == 0:  # trailing pad
            at += 1
            continue
        yield buf[at + 1], buf[at + 2 : at + 1 + length]
        at += 1 + length


def symbols(records: list[omf.Record]) -> bytes:
    """The $$SYMBOLS segment's assembled bytes, or b"" if BC wasn't asked for /Zi."""
    for index, segment in enumerate(omf.segments(records)):
        if segment and segment[0] == "$$SYMBOLS":
            return omf.segment_image(records, index, segment[1])
    return b""


def module_name(records: list[omf.Record]) -> str | None:
    """THEADR's own name, rather than the $$SYMBOLS module-open record.

    QB 4.5 doesn't emit that record's filename field at all -- its module-open
    record is four bytes, none of them a name -- so THEADR is the one source
    all three compilers agree on.
    """
    for r in records:
        if r.type == omf.THEADR:
            n = r.body[0]
            return r.body[1 : 1 + n].decode("latin1")
    return None


def parse(records: list[omf.Record]) -> DebugInfo:
    buf = symbols(records)
    module = module_name(records) if buf else None
    procedures: list[Procedure] = []
    variables: list[Variable] = []
    labels: list[Label] = []
    current: Procedure | None = None

    for kind, data in _records(buf):
        if kind == BLOCK:
            pass  # module-open record: shape (and presence of a name) varies by compiler
        elif kind == PROC:
            # data[2:4] is unaccounted for -- plausibly a pre-link proctype
            # index, mirroring how BPREL/LDATA carry their own such index.
            off = int.from_bytes(data[0:2], "little")
            proc_length = int.from_bytes(data[4:6], "little")
            debug_start = int.from_bytes(data[6:8], "little")
            debug_end = int.from_bytes(data[8:10], "little")
            flags = data[12]
            name, _ = _pstr(data, 13)
            current = Procedure(name, off, proc_length, debug_start, debug_end, flags)
            procedures.append(current)
        elif kind == END:
            current = None
        elif kind == BPREL and current is not None:
            bp_offset = int.from_bytes(data[0:2], "little", signed=True)
            type_index = int.from_bytes(data[2:4], "little")
            name, _ = _pstr(data, 4)
            current.locals.append(Local(name, bp_offset, type_index))
        elif kind == LDATA:
            off, seg, type_index = (
                int.from_bytes(data[0:2], "little"),
                int.from_bytes(data[2:4], "little"),
                int.from_bytes(data[4:6], "little"),
            )
            name, _ = _pstr(data, 6)
            variables.append(Variable(name, off, seg, type_index))
        elif kind == LABEL:
            off = int.from_bytes(data[0:2], "little")
            name, _ = _pstr(data, 3)
            labels.append(Label(name, off))

    return DebugInfo(module, procedures, variables, labels)


def _fmt_type(type_index: int, resolved: str | None) -> str:
    return resolved if resolved else f"custom (type {type_index:#06x}, unresolved)"


def main(path: Path | str) -> None:
    info = parse(omf.read(path))
    print(path)
    if info.module is None:
        print("  no /Zi debug info ($$SYMBOLS is empty)")
        return
    print(f"  module: {info.module}")
    for proc in info.procedures:
        params = ", ".join(f"{p.name}:{_fmt_type(p.type_index, p.type_name)}" for p in proc.params)
        ret = f" returns {proc.return_type}" if proc.return_type else ""
        print(f"  sub/function {proc.name}{ret}  off={proc.offset:#06x} len={proc.proc_length} flags={proc.flags:#x}")
        print(f"    params: {params or '(none)'}")
        for loc in proc.own_locals:
            print(f"    local  {loc.name:<12} bp={loc.bp_offset:+5d}  {_fmt_type(loc.type_index, loc.type_name)}")
    for v in info.variables:
        print(f"  var   {v.name:<12} seg={v.segment:3d} off={v.offset:#06x}  {_fmt_type(v.type_index, v.type_name)}")
    for lbl in info.labels:
        print(f"  label {lbl.name:<12} off={lbl.offset:#06x}")


if __name__ == "__main__":
    import sys

    for p in sys.argv[1:]:
        main(p)
