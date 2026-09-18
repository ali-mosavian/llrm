"""The one OMF emitter, shared by the C and BC-object frontends.

The C adapter consumes a ``masm.Module``.  The BC adapter consumes decoded
segment, symbol and relocation semantics plus freshly laid-out code.  Both
construct a complete object here; neither invokes an assembler or rewrites an
input record stream.  A reference to anything the C module defines is a fixup
against its segment with the addend in the code, as jwasm writes it; anything
else names its EXTDEF.
"""

import struct
from dataclasses import field
from dataclasses import dataclass
from collections.abc import Sequence

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import masm
from qbopt.backend import layout
from qbopt.backend import select
from qbopt.objectfile import omf
from qbopt.objectfile.module import Space
from qbopt.objectfile.module import Module
from qbopt.objectfile.module import SourceMap

OFFSET, BASE, POINTER = 1, 2, 3  # OMF locations: offset16, segment base, ptr16:16
WIDE = {OFFSET: 2, BASE: 2, POINTER: 4}
CLASSES = {"_DATA": "DATA", "_BSS": "BSS", "CONST": "CONST"}
GROUP = 1  # DGROUP, the only group
# LEDATA payload per record. A fixup's offset into its record has ten bits.
CHUNK = 1000
ACBP = 0x48  # relocatable, word aligned, public, 16-bit
SEGMENT_TARGET, GROUP_TARGET, EXTERNAL_TARGET = 0, 1, 2
GROUP_FRAME, TARGET_FRAME = 1, 5


class Unencodable(Exception):
    """An instruction or reference this writer has no bytes for."""


class Survived(Exception):
    """A phi reached emission. Always a bug in phi elimination."""


@dataclass(frozen=True, slots=True)
class Fixup:
    at: int
    loc: int
    name: str
    relative: bool = False


@dataclass(slots=True)
class Piece:
    code: bytes
    fixups: tuple[Fixup, ...] = ()  # `at` relative to the piece


@dataclass(slots=True)
class Jump:
    name: str
    label: str
    long: bool = False


@dataclass(frozen=True, slots=True)
class Near:
    name: str


type Encoded = masm.Label | Piece | Jump | Near


@dataclass(slots=True)
class Segment:
    name: str
    klass: str
    grouped: bool
    image: bytearray = field(default_factory=bytearray)
    spans: list[list[int]] = field(default_factory=list)  # [start, end) holding data
    fixups: list[Fixup] = field(default_factory=list)

    def put(self, code: bytes, fixups: tuple[Fixup, ...] = ()) -> None:
        at = len(self.image)
        self.fixups += [Fixup(at + one.at, one.loc, one.name, one.relative) for one in fixups]
        self.image += code
        if self.spans and self.spans[-1][1] == at:
            self.spans[-1][1] += len(code)
        elif code:
            self.spans.append([at, at + len(code)])

    def skip(self, size: int) -> None:
        self.image += bytes(size)


def written_bc(
    found: Module,
    bodies: "list[lir.LirBody]",
    records: list[omf.Record],
    assignment: dict,
    tables: tuple = (),
    fields: frozenset[int] = frozenset(),
    reached: frozenset[int] | None = None,
    native_fpu: bool = False,
    ordered: bool = False,
    source: SourceMap | None = None,
) -> bytes | str:
    """Write a complete fresh object for the BC-object frontend.

    BC's OBJ is input syntax here: its declarations, data and relocations are
    decoded, just as C source is decoded by the other frontend.  The output is
    never made by splicing LEDATA or FIXUPP records back into BC's record
    stream; ``_bc_records`` serializes a new stream from those semantics.
    """
    _require_no_phis(bodies)
    if bodies and all(body.ordered for body in bodies):
        ordered = True
    laid = layout.rebuild(
        found,
        [(one.name, one) for one in bodies],
        tables,
        fields,
        reached,
        native_fpu,
        assignment=assignment or None,
        ordered=ordered,
        ordered_entries=frozenset(body.entry for body in bodies if body.ordered),
        source=source,
    )
    if isinstance(laid, str):
        return laid

    kept = min(body.entry for body in bodies)
    image = found.code[:kept] + laid.code
    relocations: dict[int, list[int]] = {}
    for new, old in laid.relocations:
        relocations.setdefault(old, []).append(kept + new)
    groups = list(omf.groups(records))
    # A SEGMENT-space address outside DGROUP is an offset in its owning
    # segment, even when the instruction carries no explicit segment
    # override.  ``segment`` records the latter machine detail; it does not
    # turn a code or private-data symbol into a DGROUP address.  Fresh OMF
    # therefore frames those references at their target segment.
    target_segment = lambda address: address.space is Space.SEGMENT and address.index not in found.dgroup
    group_framed = [
        address for _offset, address in laid.symbols if address.segment == Register.NONE and not target_segment(address)
    ]
    if group_framed and "DGROUP" not in groups:
        return "generated data references require an established DGROUP frame"
    added = []
    for offset, address in laid.symbols:
        target = "segment" if address.space is Space.SEGMENT else "external"
        fixup = (
            omf.target_offset_fixup(found.seg, kept + offset, target, address.index, address.disp)
            if address.segment != Register.NONE or target_segment(address)
            else omf.offset_fixup(
                found.seg,
                kept + offset,
                target,
                address.index,
                address.disp,
                groups.index("DGROUP") + 1,
            )
        )
        added.append((kept + offset, fixup))
    moved = {**laid.covered, **laid.moved}
    return _bc_object(
        records,
        found.seg,
        kept,
        image,
        moved,
        {old: tuple(dict.fromkeys(destinations)) for old, destinations in relocations.items()},
        laid.dropped,
        tuple(added),
    )


def _require_no_phis(bodies: "Sequence[lir.LirBody]") -> None:
    """Reject SSA joins before anything attempts to encode instructions."""
    stuck = [block.at for body in bodies for block in body.blocks if block.phis]
    if stuck:
        raise Survived(
            "a phi survives at " + ", ".join(f"{one:#06x}" for one in stuck) + "; nothing below can emit one"
        )


def _mapped(offset: int, kept: int, moved: dict[int, int]) -> int | None:
    return offset if offset < kept else moved.get(offset)


def _bc_object(
    records: list[omf.Record],
    code_seg: int,
    kept: int,
    code: bytes,
    moved: dict[int, int],
    relocations: dict[int, tuple[int, ...]],
    dropped: frozenset[int],
    added: tuple[tuple[int, omf.Fixup], ...],
) -> bytes | str:
    """Canonical OMF serialization of one decoded BC module.

    Segment and symbol indices deliberately retain the frontend's numbering;
    they are identities in decoded FIXUPP semantics, not positions borrowed
    from the old output stream.  Record boundaries and ordering are ours.
    """
    segments = omf.segments(records)
    if not 0 < code_seg < len(segments) or segments[code_seg] is None:
        return "the module has no code segment"
    if any(omf.has_start_address(one) for one in records if one.type & 0xFE == omf.MODEND):
        return "MODEND carries a start address, which this does not move yet"
    supported = {
        omf.THEADR,
        omf.COMENT,
        omf.MODEND,
        omf.EXTDEF,
        omf.PUBDEF,
        omf.LINNUM,
        omf.LNAMES,
        omf.SEGDEF,
        omf.GRPDEF,
        omf.FIXUPP,
        omf.LEDATA,
    }
    unknown = [one.name for one in records if one.type & 0xFE not in supported]
    if unknown:
        return f"fresh OMF emission does not model {unknown[0]}"

    images: dict[int, bytes] = {}
    spans: dict[int, list[tuple[int, int]]] = {}
    for index, segment in enumerate(segments):
        if index == 0 or segment is None:
            continue
        images[index] = code if index == code_seg else omf.segment_image(records, index, segment[1])
        pieces = (
            [(0, len(code))]
            if index == code_seg and code
            else [(at, at + len(payload)) for _record, seg, at, payload in omf.ledata(records) if seg == index]
        )
        spans[index] = _merged(pieces)

    placed: dict[int, list[tuple[int, omf.Fixup, int]]] = {index: [] for index in images}
    for fixup in omf.fixups(records):
        if fixup.seg is None or fixup.seg not in placed:
            return "a fixup has no segment to attach to"
        destinations: tuple[int, ...]
        if fixup.seg == code_seg:
            landed = relocations.get(fixup.offset) if fixup.offset >= kept else (fixup.offset,)
            if landed is None:
                if fixup.offset in dropped:
                    continue
                return f"the fixup at {fixup.offset:#x} has nowhere to go in the rebuilt segment"
            destinations = landed
        else:
            destinations = (fixup.offset,)
        disp = fixup.disp
        if fixup.target == "segment" and fixup.index == code_seg and fixup.disp_pos is not None:
            mapped = _mapped(disp, kept, moved)
            if mapped is None:
                return f"a fixup names {disp:#x}, which is not an instruction the layout placed"
            disp = mapped
        for destination in destinations:
            placed[fixup.seg].append((destination, fixup, disp))
    for destination, fixup in added:
        placed[code_seg].append((destination, fixup, fixup.disp))

    headers: list[omf.Record] = []
    first = next((one for one in records if one.type & 0xFE == omf.THEADR), None)
    if first is None:
        return "the module has no THEADR"
    headers.append(omf.Record(first.type, first.body))
    headers += [omf.Record(one.type, one.body) for one in records if one.type & 0xFE == omf.COMENT]

    lnames = omf.names(records)[1:]
    headers.append(omf.Record(omf.LNAMES, b"".join(_string(one) for one in lnames)))

    seg_index = 0
    for one in records:
        if one.type & 0xFE != omf.SEGDEF:
            continue
        seg_index += 1
        body = bytearray(one.body)
        if seg_index == code_seg:
            at = omf.segment_length_at(one)
            struct.pack_into("<H", body, at, len(code) & 0xFFFF)
            if len(code) == 0x10000:
                body[0] |= 0x02
            else:
                body[0] &= ~0x02
        headers.append(omf.Record(one.type, bytes(body)))
    headers += [omf.Record(one.type, one.body) for one in records if one.type & 0xFE == omf.GRPDEF]
    headers += [omf.Record(one.type, one.body) for one in records if one.type & 0xFE == omf.EXTDEF]

    for one in records:
        if one.type & 0xFE not in (omf.PUBDEF, omf.LINNUM):
            continue
        try:
            changes = {
                at: _mapped(struct.unpack_from("<H", one.body, at)[0], kept, moved)
                for at in omf.code_offsets(one, code_seg)
            }
        except struct.error:
            return "a record names a code offset past its own end"
        if any(value is None for value in changes.values()):
            return "a symbol or line names code that the layout did not place"
        patched = omf.patched(one, {at: value for at, value in changes.items() if value is not None})
        headers.append(omf.Record(patched.type, patched.body))

    data: list[omf.Record] = []
    for index in range(1, len(segments)):
        if segments[index] is None:
            continue
        made = _fresh_segment(index, images[index], spans[index], placed[index])
        if isinstance(made, str):
            return made
        data += made
    end = next((one for one in reversed(records) if one.type & 0xFE == omf.MODEND), None)
    if end is None:
        return "the module has no MODEND"
    fresh = [*headers, *data, omf.Record(end.type, end.body)]
    return b"".join(one.emit() for one in fresh)


def _merged(spans: list[tuple[int, int]]) -> list[tuple[int, int]]:
    out: list[tuple[int, int]] = []
    for lo, hi in sorted(spans):
        if lo == hi:
            continue
        if out and lo <= out[-1][1]:
            out[-1] = out[-1][0], max(out[-1][1], hi)
        else:
            out.append((lo, hi))
    return out


def _fresh_segment(
    index: int,
    image: bytes,
    spans: list[tuple[int, int]],
    fixups: list[tuple[int, omf.Fixup, int]],
) -> list[omf.Record] | str:
    """Canonical LEDATA/FIXUPP records for one semantic segment."""
    widths = {0: 1, 1: 2, 2: 2, 3: 4, 4: 1, 5: 2, 9: 4, 11: 6, 13: 4}
    if any(one.loc not in widths for _at, one, _disp in fixups):
        return "unsupported relocation field width"
    out: list[omf.Record] = []
    fixups.sort(key=lambda item: item[0])
    placed = 0
    for span_lo, span_hi in spans:
        start = span_lo
        while start < span_hi:
            stop = min(span_hi, start + CHUNK)
            crossing = [at for at, one, _disp in fixups if at < stop < at + widths[one.loc]]
            if crossing:
                stop = min(crossing)
            if stop <= start:
                return f"segment {index}: a relocation field cannot fit in LEDATA"
            out.append(omf.ledata_record(index, start, image[start:stop]))
            mine = [(at, one, disp) for at, one, disp in fixups if start <= at < stop]
            if mine:
                out.append(omf.fixupp_record([_resolved_fixup(one, at - start, disp) for at, one, disp in mine]))
            placed += len(mine)
            start = stop
    if placed != len(fixups):
        return f"segment {index}: a fixup lies outside initialized data"
    return out


def _resolved_fixup(one: omf.Fixup, offset: int, disp: int) -> bytes:
    """Encode a decoded fixup explicitly, with no dependency on THREAD state."""
    if not 0 <= offset < 1024:
        raise ValueError(f"a fixup offset is ten bits; {offset:#x} does not fit")
    target_method = {"segment": 0, "group": 1, "external": 2}.get(one.target)
    if target_method is None:
        raise ValueError(f"unsupported fixup target {one.target}")
    if isinstance(one.frame, omf.Thread):
        frame_method, frame_index = one.frame.method, one.frame.index
    elif isinstance(one.frame, int):
        frame_method, frame_index = one.frame_method, one.frame
    else:
        frame_method, frame_index = 5, None  # target's frame
    if frame_method is None:
        frame_method = 5
    lead = 0x80 | (0 if one.selfrel else 0x40) | one.loc << 2 | offset >> 8
    body = bytearray([lead, offset & 0xFF, frame_method << 4 | target_method])
    if frame_method < 3:
        if frame_index is None:
            raise ValueError("an explicit fixup frame has no index")
        body += omf.as_index(frame_index)
    body += omf.as_index(one.index)
    body += struct.pack("<H", disp)
    return bytes(body)


def written(module: masm.Module, source: str) -> bytes:
    segments = [Segment(module.code, "CODE", False)]
    named = {"_DATA": Segment("_DATA", "DATA", True)}
    for name, _items in module.data:
        if name not in named:
            private = name in module.private
            named[name] = Segment(name, CLASSES.get(name, "FAR_DATA" if private else "DATA"), not private)
    segments += named.values()
    symbols: dict[str, tuple[int, int]] = {}
    for name, items in module.data:
        index = [one.name for one in segments].index(name)
        _data(segments[index], index, items, symbols)
    _code(segments[0], module, symbols)
    externs = {name: kind for name, kind in module.externs}
    return b"".join(record.emit() for record in _records(module, source, segments, symbols, externs))


def _data(segment: Segment, index: int, items: tuple[masm.Datum, ...], symbols: dict[str, tuple[int, int]]) -> None:
    for item in items:
        match item:
            case masm.Label(name=name):
                symbols[name] = (index, len(segment.image))
            case masm.Fill(size=size, byte=None):
                segment.skip(size)
            case masm.Fill(size=size, byte=byte) if byte is not None:
                segment.put(bytes([byte]) * size)
            case masm.Pointer(name=name, offset=offset, far=far):
                loc = POINTER if far else OFFSET
                segment.put(bytes(WIDE[loc]), (Fixup(0, loc, name),))
                struct.pack_into("<H", segment.image, len(segment.image) - WIDE[loc], offset & 0xFFFF)
            case masm.Align(to=to):
                segment.put(bytes(-len(segment.image) % to))
            case bytes():
                segment.put(item)


def _code(segment: Segment, module: masm.Module, symbols: dict[str, tuple[int, int]]) -> None:
    items: list[Encoded] = []
    for number, procedure in enumerate(module.procedures):
        items.append(masm.Label(procedure.name))
        for item in masm.listing(procedure, number):
            try:
                items += _items(item, module.names, number)
            except Unencodable as error:
                raise Unencodable(f"{procedure.name}: {error}") from error
    labels = _relaxed(items)
    at = 0
    for item in items:
        match item:
            case masm.Label(name=name):
                symbols[name] = (0, at)
            case Piece(code=code, fixups=fixups):
                segment.put(code, fixups)
            case Jump(name=name, label=label, long=long):
                segment.put(_jump(name, labels[label], at, long).code)
            case Near(name=name) if name in labels:
                segment.put(bytes([0xE8]) + struct.pack("<h", labels[name] - (at + 3)))
            case Near(name=name):
                segment.put(bytes(3), (Fixup(1, OFFSET, name, relative=True),))
                segment.image[at] = 0xE8
        at = len(segment.image)


def _items(item: masm.Item, names: dict[tuple[Space, int], str], number: int) -> list[Encoded]:
    match item:
        case masm.Label():
            return [item]
        case masm.Callee(code=code) if code:
            return [_part(one) for one in code]
        case masm.Callee(name=name, far=True):
            return [Piece(bytes([0x9A, 0, 0, 0, 0]), (Fixup(1, POINTER, name),))]
        case masm.Callee(name=name):
            return [Near(name)]
        case ir.Semantics(op=ir.Operation.BRANCH | ir.Operation.JUMP, name=name, target=target):
            if target is None:
                raise Unencodable(f"{name or 'jump'} with no target")
            return [Jump(name or "jmp", masm.label(number, target))]
    return [_encoded(item, names)]


def _part(part: masm.InlinePart) -> Piece:
    match part:
        case bytes():
            return Piece(part)
        case ("offset", name, offset):
            return Piece(struct.pack("<H", offset & 0xFFFF), (Fixup(0, OFFSET, name),))
        case ("segment", name, _):
            return Piece(bytes(2), (Fixup(0, BASE, name),))
    raise Unencodable(f"inline part {part}")


def _encoded(what: ir.Semantics, names: dict[tuple[Space, int], str]) -> Piece:
    relocated = any(isinstance(one, ir.Imm) and one.address is not None for one in what.sources)
    made = select.emit(what, relocated=relocated)
    if made is None:
        raise Unencodable(f"{what}")
    code, fixups = bytearray(made.code), {}
    for one in (*what.dests, *what.sources):
        match one:
            case ir.Mem(addr=addr) | ir.Address(addr=addr) if addr is not None and addr.space in (
                Space.SEGMENT,
                Space.EXTERNAL,
            ):
                at, loc, addend = made.displacement_at, OFFSET, addr.disp
            case ir.Imm(address=addr) if addr is not None and addr.space is Space.GROUP:
                at, loc, addend = made.immediate_at, BASE, 0
            case ir.Imm(address=addr, value=value) if addr is not None:
                at, loc, addend = made.immediate_at, OFFSET, addr.disp + value
            case _:
                continue
        if at is None:
            raise Unencodable(f"{what}: no field for {one}")
        struct.pack_into("<H", code, at, addend & 0xFFFF)
        fixups[at] = Fixup(at, loc, names[(addr.space, addr.index)])
    return Piece(bytes(code), tuple(fixups.values()))


def _relaxed(items: Sequence[Encoded]) -> dict[str, int]:
    """Every label's offset, with each jump short unless its target is out of reach.

    Short first and lengthened to a fixed point, as jwasm does: lengthening
    only moves targets further away, so it ends, and at the smallest layout.
    """
    while True:
        labels, at = {}, 0
        for item in items:
            if isinstance(item, masm.Label):
                labels[item.name] = at
            at += _length(item)
        changed, at = False, 0
        for item in items:
            # Measured before the jump may grow: `labels` is this pass's layout.
            length = _length(item)
            if isinstance(item, Jump) and not item.long:
                if item.label not in labels:
                    raise Unencodable(f"a jump to {item.label}, which is nowhere")
                if not -128 <= labels[item.label] - (at + 2) <= 127:
                    item.long = changed = True
            at += length
        if not changed:
            return labels


def _length(item: Encoded) -> int:
    match item:
        case masm.Label():
            return 0
        case Piece(code=code):
            return len(code)
        case Jump(name=name, long=long):
            return 2 if not long else 3 if name == "jmp" else 4
        case Near():
            return 3
    raise Unencodable(f"{item}")


def _jump(name: str, target: int, at: int, long: bool) -> select.Emitted:
    made = select.jump(target, at, not long) if name == "jmp" else select.branch(name, target, at, not long)
    if made is None or len(made.code) != _length(Jump(name, "", long)):
        raise Unencodable(f"{name} from {at:#x} to {target:#x}")
    return made


def _records(
    module: masm.Module,
    source: str,
    segments: list[Segment],
    symbols: dict[str, tuple[int, int]],
    externs: dict[str, str],
) -> list[omf.Record]:
    lnames = [""]

    def lname(text: str) -> int:
        lnames.append(text)
        return len(lnames)

    segdefs = []
    for segment in segments:
        klass, name = lname(segment.klass), lname(segment.name)
        size = len(segment.image)
        acbp = ACBP | (2 if size == 0x10000 else 0)
        body = bytes([acbp]) + struct.pack("<H", size & 0xFFFF) + _names(name, klass, 1)
        segdefs.append(omf.Record(omf.SEGDEF, body))
    grouped = [index for index, segment in enumerate(segments, 1) if segment.grouped]
    grpdef = omf.Record(omf.GRPDEF, _names(lname("DGROUP")) + b"".join(b"\xff" + omf.as_index(one) for one in grouped))

    used = {_target(one.name) for segment in segments for one in segment.fixups} - symbols.keys() - {"DGROUP"}
    if missing := used - externs.keys():
        raise Unencodable(f"references to nothing defined or declared: {sorted(missing)}")
    # masm.text's order, data externals first; LINK searches libraries in EXTDEF order.
    declared = sorted(externs, key=lambda name: externs[name] != "byte")
    order = [name for name in declared if name in used]
    data = []
    for index, segment in enumerate(segments, 1):
        data += _ledata(index, segment, symbols, segments, {name: n for n, name in enumerate(order, 1)}, externs)

    records = [omf.Record(omf.THEADR, _string(source)), omf.Record(omf.LNAMES, b"".join(map(_string, lnames)))]
    records += [*segdefs, grpdef]
    if order:
        records.append(omf.Record(omf.EXTDEF, b"".join(_string(name) + b"\x00" for name in order)))
    for index, segment in enumerate(segments, 1):
        defined = [(name, at) for name, (seg, at) in symbols.items() if seg == index - 1 and name in module.publics]
        if defined:
            head = bytes([GROUP if segment.grouped else 0]) + omf.as_index(index)
            names = b"".join(_string(n) + struct.pack("<HB", at, 0) for n, at in defined)
            records.append(omf.Record(omf.PUBDEF, head + names))
    return [*records, *data, omf.Record(omf.MODEND, b"\x00")]


def _ledata(
    index: int,
    segment: Segment,
    symbols: dict[str, tuple[int, int]],
    segments: list[Segment],
    externs: dict[str, int],
    kinds: dict[str, str],
) -> list[omf.Record]:
    fixups = sorted(segment.fixups, key=lambda one: one.at)
    subrecords = {one.at: _subrecord(one, segment, symbols, segments, externs, kinds) for one in fixups}
    out, placed = [], 0
    for start, end in segment.spans:
        while start < end:
            stop = min(end, start + CHUNK)
            for one in fixups:
                if one.at < stop < one.at + WIDE[one.loc]:
                    stop = one.at
            out.append(omf.ledata_record(index, start, bytes(segment.image[start:stop])))
            inside = [one for one in fixups if start <= one.at < stop]
            if inside:
                out.append(omf.fixupp_record([_located(subrecords[one.at], one, start) for one in inside]))
            placed += len(inside)
            start = stop
    if placed != len(fixups) or len(subrecords) != len(fixups):
        raise Unencodable(f"{segment.name}: a fixup outside the data, or two in one field")
    return out


def _subrecord(
    one: Fixup,
    segment: Segment,
    symbols: dict[str, tuple[int, int]],
    segments: list[Segment],
    externs: dict[str, int],
    kinds: dict[str, str],
) -> bytes:
    """Everything after the location: fix data, frame datum, target datum."""
    name = _target(one.name)
    if name == "DGROUP":
        method, datum, grouped = GROUP_TARGET, GROUP, True
    elif name in symbols:
        seg, at = symbols[name]
        method, datum, grouped = SEGMENT_TARGET, seg + 1, segments[seg].grouped
        if one.loc in (OFFSET, POINTER) and not one.relative:
            (addend,) = struct.unpack_from("<H", segment.image, one.at)
            struct.pack_into("<H", segment.image, one.at, (addend + at) & 0xFFFF)
    else:
        method, datum, grouped = EXTERNAL_TARGET, externs[name], kinds[name] == "byte"
    if one.loc == OFFSET and grouped and not one.relative:
        return bytes([GROUP_FRAME << 4 | 4 | method]) + omf.as_index(GROUP) + omf.as_index(datum)
    return bytes([TARGET_FRAME << 4 | 4 | method]) + omf.as_index(datum)


def _located(subrecord: bytes, one: Fixup, start: int) -> bytes:
    offset = one.at - start
    return bytes([0x80 | (0 if one.relative else 0x40) | one.loc << 2 | offset >> 8, offset & 0xFF]) + subrecord


def _target(name: str) -> str:
    return name.removeprefix("seg ")


def _names(*indices: int) -> bytes:
    return b"".join(omf.as_index(one) for one in indices)


def _string(text: str) -> bytes:
    encoded = text.encode("latin1")
    return bytes([len(encoded)]) + encoded
