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
0x97->0xB7, and now 0x88->0xA8, 0x89->0xA9 for SINGLE/DOUBLE, measured on
suite/byref2.bas). QB45_BYREF_PRIMITIVES is that table; STRING(far) is still
unmeasured -- forcing a far string needs `/Fs`, and `docs/inherited-plan.md`'s
own switch matrix has PDS take it and QB 4.5 reject it, so there is no QB 4.5
switch that reaches that code path at all.

BYVAL skips the wrapper chain entirely: a `BYVAL n AS LONG` parameter's own
type_index is the plain PRIMITIVES/custom code, identical to a local's --
confirmed on VBDOS and PDS (`suite/cvonly/byval.bas`; see below for why it
lives outside `suite/`). QuickBASIC 4.5 has no BYVAL at all -- `BC.EXE`
rejects the syntax outright ("Formal parameter specification illegal"), so
this is a two-compiler-only measurement and `type_name` needed no change for
it: PRIMITIVES already covers a bare primitive code.

An array parameter is BYREF on VBDOS and PDS through the exact same
`Tag.BYREF`-wraps-`Tag.POINTER` chain any other BYREF parameter uses, just
pointing at a `Tag.ARRAY` record instead of a primitive or `Tag.STRUCT` one --
no new tag. QuickBASIC 4.5 diverges a second way here: rather than its own
PRIMITIVES-plus-0x20 shortcut (which only covers INTEGER/LONG/STRING), an
array parameter gets a bare `Tag.POINTER` record as its own type_index --
one hop, skipping `Tag.BYREF` entirely. Measured on `suite/arrprm.bas` on
all three compilers.

A `TYPE` field whose own type is another `TYPE` needed no new code at all:
`_parse_struct` stores a field's type_index exactly like any other, and
`type_name`'s existing `Struct` branch already resolves it by recursing --
confirmed on `suite/nestud.bas`, both a module-level and a procedure-local
instance, plain and arrayed. The one shape nesting exposed that needed
support is `Tag.FIXED_STRING` (0x8D): a `STRING * n` field (BASIC requires
a fixed length inside a `TYPE`) gets its own $$TYPES record naming its
declared length, never a bare PRIMITIVES STRING code -- measured for two
different lengths on all three compilers. QB 4.5's own encoding of it is a
structurally different record (`Tag.FIXED_STRING_QB45`, tag 0x78): shaped
like a stunted `Tag.STRUCT` -- the same second byte 0x86 and a `size_bits`
field in the same position -- with a fixed three-byte tail that does not
vary between the two lengths measured and isn't decoded further.

**A structure record's trailing byte is 0x69 in every shape thrown at it**:
a one-field struct, a struct nested inside another, a struct whose last
field is a fixed string rather than a primitive, and a struct that is the
last record in the whole segment. Still nothing in BC's own output
distinguishes what it would mean for it to be anything else, so it stays
unread rather than guessed at.

**$$TYPES' `kind` byte is still only ever measured as 0x01** -- none of
BYVAL, an array parameter, a nested `TYPE`, or `Tag.FIXED_STRING` produced
anything else.

A FUNCTION's return type is read off BASIC's own type-suffix sigil on the
function's name where the source wrote one ($/%/&/!/#) -- reliable because
it's the source's own convention, not a reconstruction of the debug format.
A `0x01` $$SYMBOLS PROC record's own type_index (`Procedure.proc_type_index`)
names a `Tag.SIGNATURE` (0x75 0x80) record in the *same* module-local
$$TYPES table, and that record's own `return_type` field agrees with the
sigil for every FUNCTION measured (LONG, SINGLE, DOUBLE) -- wired up as
`Procedure.signature`. It does not replace the sigil: a SUB's own signature
carries the exact same value a FUNCTION implicitly returning INTEGER would
(BC's own default type under DEFINT), so nothing in this record can tell a
SUB from an INTEGER FUNCTION apart, and `return_type` keeps the sigil for
that reason.

`suite/cvonly/` holds probes that measure real, confirmed BC behaviour but
cannot compile on all three compilers (BYVAL, above) -- `tools/e2e.py`'s
`programs()` globs `suite/*.bas` directly, non-recursively, so a probe here
never enters the full differential harness, which needs to compile, link
and run on every configuration.
"""

from enum import IntEnum
from pathlib import Path
from dataclasses import field
from dataclasses import dataclass
from collections.abc import Iterator

from qbopt import omf
from qbopt import extent
from qbopt import module


# $$SYMBOLS record kinds -- see docs/codeview.md's own table for each one's data.
class Kind(IntEnum):
    BLOCK = 0x00
    PROC = 0x01
    END = 0x02
    BPREL = 0x04
    LDATA = 0x05
    LABEL = 0x0B


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
QB45_BYREF_PRIMITIVES = {0xA1: "INTEGER", 0xA2: "LONG", 0xB7: "STRING", 0xA8: "SINGLE", 0xA9: "DOUBLE"}

BASE_TYPE_INDEX = 0x0200


# $$TYPES data tags: the first byte (or two) of a record's own data, BC's
# private numbering -- confirmed by differential probing, unrelated to
# CVPACK's CV4 leaf ids of the same value.
class Tag(IntEnum):
    ARRAY = 0x8C  # element type only -- see the module docstring on bounds
    POINTER = 0x7A  # always followed by a second, constant 0x74 byte
    BYREF = 0x76  # wraps another record's index, always a POINTER one
    LIST = 0x7F  # a flat list -- of type-refs, or of named offsets
    TYPEREF = 0x83  # "a type_index follows", u16
    NAME = 0x82  # "a length-prefixed name follows"
    OFFSET = 0x85  # "a u16 numeric follows" -- a member offset, or a count
    STRUCT = 0x79  # always followed by a second, constant 0x86 byte
    FIXED_STRING = 0x8D  # VBDOS/PDS: always followed by a second, constant 0x00 byte
    # QB 4.5's own encoding of the same field, structurally unrelated: 0x86
    # (STRUCT's own second byte), a size_bits:u32 that is 8x the declared
    # length like a struct's own, then a fixed 0x83 0x80 0x00 tail that does
    # not vary with length and isn't decoded.
    FIXED_STRING_QB45 = 0x78
    SIGNATURE = 0x75  # a procedure's own return type + arglist -- always followed by a second, constant 0x80 byte


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
class FixedString:
    """A `STRING * n` field inside a TYPE -- BASIC requires a fixed length
    there, and BC gives it its own record naming that length rather than
    reusing PRIMITIVES' bare STRING code."""

    length: int


@dataclass(frozen=True, slots=True)
class Signature:
    """A procedure's own return type and argument list -- what a $$SYMBOLS
    PROC record's own type_index (Procedure.proc_type_index) names. params
    is already resolved to the argument types themselves, the same way
    Struct resolves its own field list rather than keeping the raw index."""

    return_type: int
    params: tuple[int, ...]


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


type TypeEntry = Array | Struct | Pointer | ByRef | TypeList | NamedOffsetList | FixedString | Signature | Unresolved


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
    if at + 3 > len(data) or data[at] != Tag.TYPEREF:
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
        if data[at] != Tag.NAME or at + 2 > len(data):
            return None
        namelen = data[at + 1]
        at += 2
        if at + namelen + 3 > len(data) or data[at + namelen] != Tag.OFFSET:
            return None
        name = data[at : at + namelen].decode("latin1")
        offset = int.from_bytes(data[at + namelen + 1 : at + namelen + 3], "little")
        out.append(NamedOffset(name, offset))
        at += namelen + 3
    return tuple(out)


def _parse_struct(data: bytes, table: dict[int, TypeEntry]) -> Struct | Unresolved:
    if len(data) < 17 or data[1] != 0x86 or data[6] != Tag.OFFSET or data[15] != Tag.NAME:
        return Unresolved(Tag.STRUCT, data)
    field_types_index = _type_ref(data, 9)
    field_names_index = _type_ref(data, 12)
    if field_types_index is None or field_names_index is None:
        return Unresolved(Tag.STRUCT, data)
    size_bits = int.from_bytes(data[2:6], "little")
    count = int.from_bytes(data[7:9], "little")
    field_types = table.get(field_types_index)
    field_names = table.get(field_names_index)
    namelen = data[16]
    valid_lists = isinstance(field_types, TypeList) and isinstance(field_names, NamedOffsetList)
    if len(data) < 17 + namelen or not valid_lists:
        return Unresolved(Tag.STRUCT, data)
    name = data[17 : 17 + namelen].decode("latin1")
    if len(field_types.indices) != count or len(field_names.entries) != count:
        return Unresolved(Tag.STRUCT, data)
    fields = tuple(
        Field(no.name, no.offset, ti) for ti, no in zip(field_types.indices, field_names.entries, strict=True)
    )
    return Struct(name, size_bits, fields)


def _parse_signature(data: bytes, table: dict[int, TypeEntry]) -> Signature | Unresolved:
    if len(data) != 10 or data[1] != 0x80 or data[5] != 0x73:
        return Unresolved(Tag.SIGNATURE, data)
    return_type = _type_ref(data, 2)
    nparms = data[6]
    arglist_index = _type_ref(data, 7)
    if return_type is None or arglist_index is None:
        return Unresolved(Tag.SIGNATURE, data)
    if nparms == 0 and arglist_index == BASE_TYPE_INDEX:
        # A zero-parameter procedure has no TypeList of its own to point at,
        # so its arglist names the segment's own first (always 1-byte, 0x80)
        # entry instead -- confirmed by suite/arrudt.bas and suite/nestud.bas's
        # zero-argument Inside, and absent whenever a module has no procedure
        # at all (suite/arrays.bas, suite/udt.bas never reach index 0x0201).
        return Signature(return_type, ())
    arglist = table.get(arglist_index)
    if not isinstance(arglist, TypeList) or len(arglist.indices) != nparms:
        return Unresolved(Tag.SIGNATURE, data)
    return Signature(return_type, arglist.indices)


def _parse_type_entry(kind: int, data: bytes, table: dict[int, TypeEntry]) -> TypeEntry:
    tag = data[0] if data else None
    if kind != 0x01 or tag is None:
        return Unresolved(kind, data)
    match tag:
        case Tag.ARRAY:
            ref = _type_ref(data, 1)
            return Array(ref) if ref is not None else Unresolved(tag, data)
        case Tag.POINTER if len(data) >= 2 and data[1] == 0x74:
            ref = _type_ref(data, 2)
            return Pointer(ref) if ref is not None else Unresolved(tag, data)
        case Tag.BYREF:
            ref = _type_ref(data, 1)
            return ByRef(ref) if ref is not None else Unresolved(tag, data)
        case Tag.LIST:
            rest = data[1:]
            if rest and rest[0] == Tag.NAME:
                offsets = _named_offsets(rest)
                return NamedOffsetList(offsets) if offsets is not None else Unresolved(tag, data)
            refs = _type_refs(rest)
            return TypeList(refs) if refs is not None else Unresolved(tag, data)
        case Tag.STRUCT:
            return _parse_struct(data, table)
        case Tag.FIXED_STRING if len(data) >= 5 and data[1] == 0x00 and data[2] == Tag.OFFSET:
            return FixedString(int.from_bytes(data[3:5], "little"))
        case Tag.FIXED_STRING_QB45 if len(data) == 9 and data[1] == 0x86 and data[6:9] == b"\x83\x80\x00":
            size_bits = int.from_bytes(data[2:6], "little")
            return FixedString(size_bits // 8) if size_bits % 8 == 0 else Unresolved(tag, data)
        case Tag.SIGNATURE:
            return _parse_signature(data, table)
        case _:
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
    match types.get(type_index):
        case Array(element=element):
            return f"ARRAY OF {type_name(element, types) or f'type {element:#06x}'}"
        case Struct(name=name):
            return f"TYPE {name}"
        case ByRef(target=target):
            pointer = types.get(target)
            pointee = pointer.target if isinstance(pointer, Pointer) else target
            return f"BYREF {type_name(pointee, types) or f'type {pointee:#06x}'}"
        case Pointer(target=target):
            # QB 4.5's own array-parameter shape: a bare pointer, no Tag.BYREF
            # hop -- see the module docstring. Still BYREF in BASIC's own terms,
            # since an array parameter is never anything else.
            return f"BYREF {type_name(target, types) or f'type {target:#06x}'}"
        case FixedString(length=length):
            return f"STRING * {length}"
        case _:
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
    # a $$TYPES index naming this procedure's own Tag.SIGNATURE record --
    # see Procedure.signature and the module docstring.
    proc_type_index: int
    locals: list[Local] = field(default_factory=list)
    types: dict[int, TypeEntry] = field(default_factory=dict)

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

    @property
    def signature(self) -> "Signature | None":
        """The procedure's own Tag.SIGNATURE record, if proc_type_index
        resolves to one. Its return_type agrees with the sigil for every
        FUNCTION measured, but a SUB's own signature carries the same
        placeholder an INTEGER FUNCTION's would -- nothing here can tell the
        two apart, so this is not folded into return_type."""
        entry = self.types.get(self.proc_type_index)
        return entry if isinstance(entry, Signature) else None


@dataclass(frozen=True, slots=True)
class DebugInfo:
    module: str | None
    procedures: list[Procedure]
    variables: list[Variable]
    labels: list[Label]
    types: dict[int, TypeEntry] = field(default_factory=dict)
    code_length: int = 0


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
        match kind:
            case Kind.BLOCK:
                pass  # module-open record: shape (and presence of a name) varies by compiler
            case Kind.PROC:
                # data[10:12] is still unaccounted for -- always 0x0000 across
                # every procedure measured (see the module docstring), so
                # nothing here distinguishes what a nonzero value would mean.
                off = int.from_bytes(data[0:2], "little")
                proc_type_index = int.from_bytes(data[2:4], "little")
                proc_length = int.from_bytes(data[4:6], "little")
                debug_start = int.from_bytes(data[6:8], "little")
                debug_end = int.from_bytes(data[8:10], "little")
                flags = data[12]
                name, _ = _pstr(data, 13)
                current = Procedure(name, off, proc_length, debug_start, debug_end, flags, proc_type_index, types=types)
                procedures.append(current)
            case Kind.END:
                current = None
            case Kind.BPREL if current is not None:
                bp_offset = int.from_bytes(data[0:2], "little", signed=True)
                type_index = int.from_bytes(data[2:4], "little")
                name, _ = _pstr(data, 4)
                current.locals.append(Local(name, bp_offset, type_index, types))
            case Kind.LDATA:
                off, seg, type_index = (
                    int.from_bytes(data[0:2], "little"),
                    int.from_bytes(data[2:4], "little"),
                    int.from_bytes(data[4:6], "little"),
                )
                name, _ = _pstr(data, 6)
                variables.append(Variable(name, off, seg, type_index, types))
            case Kind.LABEL:
                off = int.from_bytes(data[0:2], "little")
                name, _ = _pstr(data, 3)
                labels.append(Label(name, off))

    code_segment = omf.code_segment(records)
    code_length = code_segment[2] if code_segment else 0
    return DebugInfo(module, procedures, variables, labels, types, code_length)


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


def _fmt_body(body: extent.Body) -> str:
    ranges = ", ".join(f"{lo:#06x}-{hi:#06x}" for lo, hi in body.ranges)
    label = body.name or body.kind.value
    return f"  {body.kind.value:9} {label:<12} {ranges}  ({body.length} bytes)"


def main(path: Path | str) -> None:
    records = omf.read(path)
    print(path)

    # No CodeView needed for this part -- extent.py's own reachability answers
    # it, and does so for every real object in the corpus, /Zi or not.
    found = module.of(records)
    if found is None:
        print("  no code segment")
    else:
        found_partition = extent.partition(found)
        if isinstance(found_partition, str):
            print(f"  body partition refused: {found_partition}")
        else:
            for body in found_partition.bodies:
                print(_fmt_body(body))

    info = parse(records)
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
