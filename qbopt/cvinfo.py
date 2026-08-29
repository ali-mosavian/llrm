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
that table. Anything else -- an array, a user-defined TYPE, or a BYREF
parameter's pointer wrapper -- is a module-local index into $$TYPES.

$$TYPES has no per-entry length the way $$SYMBOLS does, but it is not
length-less either: each record is `[kind:u8][length:u16][data]`, kind
always measured as 0x01, walked the same way as $$SYMBOLS starting from
index 0x0200. What is length-less is `data` itself -- CVPACK's own CV4 leaf
grammar (LF_POINTER, LF_ARGLIST, LF_STRUCTURE, ...) does NOT carry over to
these bytes; BC's own pre-link tag bytes (0x8C array, 0x79 structure, 0x7F
type-ref list, 0x76/0x7A byref-of-pointer, ...) are a completely different,
private numbering, confirmed leaf by leaf against a dozen probe programs
compiled on all three compilers (see docs/codeview.md's $$TYPES section) --
not assumed to mirror the packed side just because $$SYMBOLS did. An array's
record carries only its element type, never bounds: measured identical for
a 1-D and a 2-D DIM of the same element type, so BASIC's array bounds live
in the runtime's own array descriptor, not in debug type info, and that is
not a gap here either.

QuickBASIC 4.5 does not build a $$TYPES chain for a BYREF LONG/INTEGER/
STRING parameter at all -- VBDOS and PDS do -- it reuses the PRIMITIVES byte
plus 0x20 as a parameter type_index directly (0x81->0xA1, 0x82->0xA2,
0x97->0xB7), measured on suite/procs.bas. QB45_BYREF_PRIMITIVES is that
table; it is not extended to SINGLE/DOUBLE/STRING(far), which were not
measured this way.

A FUNCTION's return type is read off BASIC's own type-suffix sigil on the
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

# QB 4.5's own BYREF-parameter codes -- see the module docstring. Not a
# $$TYPES index at all; it never leaves the PRIMITIVES-sized number space.
QB45_BYREF_PRIMITIVES = {0xA1: "INTEGER", 0xA2: "LONG", 0xB7: "STRING"}

BASE_TYPE_INDEX = 0x0200

# $$TYPES data tags: the first byte (or two) of a record's own data, BC's
# private numbering -- confirmed by differential probing, unrelated to
# CVPACK's CV4 leaf ids of the same value.
TAG_ARRAY = 0x8C  # element type only -- see the module docstring on bounds
TAG_POINTER = 0x7A  # always followed by a second, constant 0x74 byte
TAG_BYREF = 0x76  # wraps another record's index, always a TAG_POINTER one
TAG_LIST = 0x7F  # a flat list -- of type-refs, or of named offsets
TAG_TYPEREF = 0x83  # "a type_index follows", u16
TAG_NAME = 0x82  # "a length-prefixed name follows"
TAG_OFFSET = 0x85  # "a u16 numeric follows" -- a member offset, or a count
TAG_STRUCT = 0x79  # always followed by a second, constant 0x86 byte


@dataclass(frozen=True, slots=True)
class Field:
    """One structure member: BC lists field types and field names/offsets
    as two separate, parallel records, zipped back together positionally."""

    name: str
    offset: int
    type_index: int


@dataclass(frozen=True, slots=True)
class Struct:
    name: str
    size_bits: int
    fields: tuple[Field, ...]


@dataclass(frozen=True, slots=True)
class Array:
    element: int


@dataclass(frozen=True, slots=True)
class Pointer:
    target: int


@dataclass(frozen=True, slots=True)
class ByRef:
    """A BYREF parameter's own wrapper. Every one measured points at a
    Pointer record in turn -- BASIC has no syntax for a bare pointer, so
    that second hop is resolved away rather than surfaced as its own thing.
    """

    target: int


@dataclass(frozen=True, slots=True)
class TypeList:
    """A flat list of type indices -- a structure's field types, or a
    procedure's own argument list."""

    indices: tuple[int, ...]


@dataclass(frozen=True, slots=True)
class NamedOffset:
    name: str
    offset: int


@dataclass(frozen=True, slots=True)
class NamedOffsetList:
    """A structure's field names and offsets, positionally parallel to the
    TypeList naming the same fields' types."""

    entries: tuple[NamedOffset, ...]


@dataclass(frozen=True, slots=True)
class Unresolved:
    """A record whose tag byte, or whose data's own shape, wasn't confirmed
    against a compiled probe -- reported rather than guessed at."""

    tag: int
    raw: bytes


type TypeEntry = Array | Struct | Pointer | ByRef | TypeList | NamedOffsetList | Unresolved


def types(records: list[omf.Record]) -> bytes:
    """The $$TYPES segment's assembled bytes, or b"" if BC wasn't asked for /Zi."""
    for index, segment in enumerate(omf.segments(records)):
        if segment and segment[0] == "$$TYPES":
            return omf.segment_image(records, index, segment[1])
    return b""


def _type_records(buf: bytes) -> "Iterator[tuple[int, int, bytes]]":
    """(type_index, kind, data) for each record, indices from BASE_TYPE_INDEX.

    No trailing pad here the way $$SYMBOLS has one -- measured across every
    probe, the segment is consumed exactly to its declared length.
    """
    at, index = 0, BASE_TYPE_INDEX
    while at + 3 <= len(buf):
        kind = buf[at]
        length = int.from_bytes(buf[at + 1 : at + 3], "little")
        yield index, kind, buf[at + 3 : at + 3 + length]
        at += 3 + length
        index += 1


def _type_ref(data: bytes, at: int) -> int | None:
    if at + 3 > len(data) or data[at] != TAG_TYPEREF:
        return None
    return int.from_bytes(data[at + 1 : at + 3], "little")


def _type_refs(data: bytes) -> tuple[int, ...] | None:
    if len(data) % 3:
        return None
    refs: list[int] = []
    for at in range(0, len(data), 3):
        ref = _type_ref(data, at)
        if ref is None:
            return None
        refs.append(ref)
    return tuple(refs)


def _named_offsets(data: bytes) -> tuple[NamedOffset, ...] | None:
    out: list[NamedOffset] = []
    at = 0
    while at < len(data):
        if data[at] != TAG_NAME or at + 2 > len(data):
            return None
        namelen = data[at + 1]
        at += 2
        if at + namelen + 3 > len(data) or data[at + namelen] != TAG_OFFSET:
            return None
        name = data[at : at + namelen].decode("latin1")
        offset = int.from_bytes(data[at + namelen + 1 : at + namelen + 3], "little")
        out.append(NamedOffset(name, offset))
        at += namelen + 3
    return tuple(out)


def _parse_struct(data: bytes, table: dict[int, TypeEntry]) -> Struct | Unresolved:
    if len(data) < 17 or data[1] != 0x86 or data[6] != TAG_OFFSET or data[15] != TAG_NAME:
        return Unresolved(TAG_STRUCT, data)
    field_types_index = _type_ref(data, 9)
    field_names_index = _type_ref(data, 12)
    if field_types_index is None or field_names_index is None:
        return Unresolved(TAG_STRUCT, data)
    size_bits = int.from_bytes(data[2:6], "little")
    count = int.from_bytes(data[7:9], "little")
    field_types = table.get(field_types_index)
    field_names = table.get(field_names_index)
    namelen = data[16]
    valid_lists = isinstance(field_types, TypeList) and isinstance(field_names, NamedOffsetList)
    if len(data) < 17 + namelen or not valid_lists:
        return Unresolved(TAG_STRUCT, data)
    name = data[17 : 17 + namelen].decode("latin1")
    if len(field_types.indices) != count or len(field_names.entries) != count:
        return Unresolved(TAG_STRUCT, data)
    fields = tuple(
        Field(no.name, no.offset, ti) for ti, no in zip(field_types.indices, field_names.entries, strict=True)
    )
    return Struct(name, size_bits, fields)


def _parse_type_entry(kind: int, data: bytes, table: dict[int, TypeEntry]) -> TypeEntry:
    tag = data[0] if data else None
    if kind != 0x01 or tag is None:
        return Unresolved(kind, data)
    if tag == TAG_ARRAY:
        ref = _type_ref(data, 1)
        return Array(ref) if ref is not None else Unresolved(tag, data)
    if tag == TAG_POINTER and len(data) >= 2 and data[1] == 0x74:
        ref = _type_ref(data, 2)
        return Pointer(ref) if ref is not None else Unresolved(tag, data)
    if tag == TAG_BYREF:
        ref = _type_ref(data, 1)
        return ByRef(ref) if ref is not None else Unresolved(tag, data)
    if tag == TAG_LIST:
        rest = data[1:]
        if rest and rest[0] == TAG_NAME:
            offsets = _named_offsets(rest)
            return NamedOffsetList(offsets) if offsets is not None else Unresolved(tag, data)
        refs = _type_refs(rest)
        return TypeList(refs) if refs is not None else Unresolved(tag, data)
    if tag == TAG_STRUCT:
        return _parse_struct(data, table)
    return Unresolved(tag, data)


def type_table(records: list[omf.Record]) -> dict[int, TypeEntry]:
    """$$TYPES, decoded to one TypeEntry per module-local index.

    A single forward pass suffices: a structure's own record always comes
    after the two list records (field types, field names+offsets) it
    refers to, in every probe measured.
    """
    table: dict[int, TypeEntry] = {}
    for index, kind, data in _type_records(types(records)):
        table[index] = _parse_type_entry(kind, data, table)
    return table


def type_name(type_index: int, types: dict[int, TypeEntry] | None = None) -> str | None:
    if type_index in PRIMITIVES:
        return PRIMITIVES[type_index]
    if type_index in QB45_BYREF_PRIMITIVES:
        return f"BYREF {QB45_BYREF_PRIMITIVES[type_index]}"
    if not types:
        return None
    entry = types.get(type_index)
    if isinstance(entry, Array):
        return f"ARRAY OF {type_name(entry.element, types) or f'type {entry.element:#06x}'}"
    if isinstance(entry, Struct):
        return f"TYPE {entry.name}"
    if isinstance(entry, ByRef):
        pointer = types.get(entry.target)
        pointee = pointer.target if isinstance(pointer, Pointer) else entry.target
        return f"BYREF {type_name(pointee, types) or f'type {pointee:#06x}'}"
    return None


@dataclass(frozen=True, slots=True)
class Local:
    """A procedure's own parameter or local: BP-relative, sign says which."""

    name: str
    bp_offset: int
    type_index: int
    # the module's own $$TYPES, shared by reference with every other Local
    # and Variable parse() builds -- not recomputed per instance.
    types: dict[int, TypeEntry] = field(default_factory=dict)

    @property
    def is_param(self) -> bool:
        return self.bp_offset > 0

    @property
    def type_name(self) -> str | None:
        return type_name(self.type_index, self.types)


@dataclass(frozen=True, slots=True)
class Variable:
    """A module-level DIM: an absolute offset into a data segment."""

    name: str
    offset: int
    segment: int
    type_index: int
    types: dict[int, TypeEntry] = field(default_factory=dict)

    @property
    def type_name(self) -> str | None:
        return type_name(self.type_index, self.types)


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
    types: dict[int, TypeEntry] = field(default_factory=dict)


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
    types = type_table(records)
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
            current.locals.append(Local(name, bp_offset, type_index, types))
        elif kind == LDATA:
            off, seg, type_index = (
                int.from_bytes(data[0:2], "little"),
                int.from_bytes(data[2:4], "little"),
                int.from_bytes(data[4:6], "little"),
            )
            name, _ = _pstr(data, 6)
            variables.append(Variable(name, off, seg, type_index, types))
        elif kind == LABEL:
            off = int.from_bytes(data[0:2], "little")
            name, _ = _pstr(data, 3)
            labels.append(Label(name, off))

    return DebugInfo(module, procedures, variables, labels, types)


def _fmt_type(type_index: int, resolved: str | None) -> str:
    return resolved if resolved else f"custom (type {type_index:#06x}, unresolved)"


def _fmt_fields(type_index: int, types: dict[int, TypeEntry], indent: str) -> list[str]:
    """A struct's own field list, name/type/offset, one line per field."""
    entry = types.get(type_index)
    if not isinstance(entry, Struct):
        return []
    return [
        f"{indent}.{f.name:<12} +{f.offset:<3d} {_fmt_type(f.type_index, type_name(f.type_index, types))}"
        for f in entry.fields
    ]


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
        for p in proc.params:
            for line in _fmt_fields(p.type_index, info.types, "      "):
                print(line)
        for loc in proc.own_locals:
            print(f"    local  {loc.name:<12} bp={loc.bp_offset:+5d}  {_fmt_type(loc.type_index, loc.type_name)}")
            for line in _fmt_fields(loc.type_index, info.types, "      "):
                print(line)
    for v in info.variables:
        print(f"  var   {v.name:<12} seg={v.segment:3d} off={v.offset:#06x}  {_fmt_type(v.type_index, v.type_name)}")
        for line in _fmt_fields(v.type_index, info.types, "        "):
            print(line)
    for lbl in info.labels:
        print(f"  label {lbl.name:<12} off={lbl.offset:#06x}")


if __name__ == "__main__":
    import sys

    for p in sys.argv[1:]:
        main(p)
