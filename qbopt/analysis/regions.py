"""
What a reference can reach, as one set instead of five fields.

`MemRef` already carries a region set, spelled five different ways --
`allocation` is an object identity, `beyond` is what a call reaches,
`excludes` is the complement of one, `space` is the kind when the byte is not
known, and `frame_bounded` is one bit of "not the frame". Each was added to
answer one pair of spaces, and `module.may_alias`'s `match (a.space, b.space)`
is the same idea again one layer down. All of them are derived here from one
lattice, so the pairs nobody wrote down are answered too.

Two references may alias when their region sets intersect. That is the whole
rule; there is no table.

A span is a byte range in a region, counted from an origin.

**Byte range, not segment.** `excludes` is a byte range and only a byte range
can hold it, and the same arithmetic then answers two frame slots and two
statics as a consequence rather than as two more arms.

**Region, as a path.** A reference that cannot name its segment still knows it
is in DGROUP, and one that knows nothing at all is the root. A coarser region
meets every finer one below it and nothing outside it -- which is how an
EXTDEF cell aliases every static and still cannot be heap storage. Written as
flat names, that last fact has to be asserted pair by pair.

**Origin, because a displacement means nothing without one.** The frame is the
stack counted from bp instead of sp, so the two cannot be compared and always
meet -- while an exclusion still speaks only for its own origin. That is what
lets a call proven clear of the caller's locals still push.

A set is what it names less what it is known to miss. Both halves are needed
and they are independent: a reference can name its byte exactly and still
carry exclusions.

## The axioms

Everything else here is arithmetic on the object. Real mode makes an address
`segment * 16 + offset`, so two byte intervals either meet or they do not, and
94% of references resolve through a fixup to an exact (segment, displacement).
These five are the assumptions, and they are the whole list. The first three
are each one region, so removing one is deleting a region rather than editing
a rule.

**1. Locals are not globals.** The stack is its own region and not part of
DGROUP. SS==DS here and the stack lives in DGROUP, so the two could coincide
and the object cannot prove they do not -- SS itself does not exist until the
runtime sets it up. What rules it out is that the stack is last in DGROUP and
grows down, so it reaches a named variable only by overflowing into it, which
is a program that has already lost. Every optimising compiler assumes this.
Refusing it costs the whole of loop-invariant code motion in any loop that
pushes an argument: one `push` makes every named load in the loop alias
something, so nothing is invariant.

**2. Heap storage is not DGROUP.** A `B$DDIM`/`B$RDIM` allocation is its own
region. Its selector is whatever the allocator returned and cannot be the
current stack segment or a program's own data segment, independently of
whether an unchecked subscript is in range within it. Bounds and object
identity are separate facts: an out-of-range subscript may leave the array,
but it cannot thereby become a write to the caller's frame.

**3. An absolute selector is not DGROUP.** A far access through a selector the
program loaded as a literal -- `DEF SEG = &HA000`, then `POKE` -- is in that
segment and no other. The loader places DGROUP; a program naming a selector in
its own code is naming hardware or an arena it was given, not the variables
the linker laid out. A selector a fixup names for a far object is that
object's segment, which the link keeps out of DGROUP, and the same holds.
This is the one that costs something when refused: every
`POKE` aliased every static, so the `DEF SEG` cell could not be forwarded to
the access that needed it.

**4. An index stays inside its own segment.** `[seg1+si+0x20]` and
`[seg2+si+0x10]` could be one byte if the link put seg2 0x10 after seg1; the
object cannot prove otherwise. Reaching it takes a subscript past the end of
its own segment, which BASIC's arrays do not allow. So an indexed reference
reaches every byte of its own segment and no other's.

**5. A scalar lvalue is reached only through a compatible type.** C's
strict-aliasing contract applies to indirect accesses as well as named
objects.  Two incompatible scalar views of the same exact address remain
conservative: that is the spelling retained for union members.  Character,
aggregate and otherwise untyped accesses carry no class and reach anything.

The first two are regions this file has always had. The third needs the
selector's value, which arrives in `known` -- an interval per value, singleton
where it is a constant.

## What the link decides

An EXTDEF names bytes another object contributes, so it is not arithmetic on
this one -- except where this object's bytes are its alone. PUBLIC and private
combine give a segment's bytes to the object that wrote them; COMMON combine
lays every object's copy over the same bytes, and a name defined elsewhere
can be in one. So an extern and a static meet only in a COMMON segment, and
`layout` -- `module.Group` -- says which those are. Without it every segment
is taken as overlaid.

Each extern is its own origin. Two symbols cannot be compared, since the link
may put one inside the other, so they meet; one symbol is displacement
arithmetic; and an exclusion can name one symbol and no other.
"""

from dataclasses import replace
from dataclasses import dataclass

from qbopt.model import memory
from qbopt.objectfile import module
from qbopt.objectfile.module import Space

# Axiom 1 lives here: the stack is its own region and not a child of DGROUP.
ROOT: tuple[str, ...] = ()
STACK = ("stack",)
DGROUP = ("dgroup",)
# Axiom 2 and axiom 3: an allocation and a literal selector are each their own
# region, beside DGROUP rather than inside it.
ALLOCATION = ("alloc",)
ABSOLUTE = ("absolute",)
# A selector a fixup names -- a far object's own segment -- is outside DGROUP
# and the stack by the link. Two such objects may share a segment, so one region.
NAMED = ("named",)
# Bytes the link places: every extern, and every COMMON-combined segment.
LINKED = (*DGROUP, "linked")

# Only the translation to provenance uses this: DGROUP less its uncaptured segments.
NONLOCAL = ("nonlocal",)

# Displacements counted from sp, from bp, and from the segment itself.
SP, BP, HERE = "sp", "bp", ""

_FLOOR, _CEILING = -(1 << 31), 1 << 31
WHOLE = (_FLOOR, _CEILING)

Span = tuple[tuple[str, ...], str, int, int]


@dataclass(frozen=True)
class RegionSet:
    """The bytes a reference may reach: `spans`, less `holes`."""

    spans: frozenset[Span]
    holes: frozenset[Span] = frozenset()

    def intersects(self, other: "RegionSet") -> bool:
        # Either side's exclusion rules out the other's byte: the fact is
        # about the pair, not about whichever reference happens to carry it.
        holes = self.holes | other.holes
        return any(_meets(one, two) for one in _surviving(self.spans, holes) for two in _surviving(other.spans, holes))


def _under(region: tuple[str, ...], other: tuple[str, ...]) -> bool:
    """Whether one region is the other or lies inside it."""
    return region[: len(other)] == other or other[: len(region)] == region


def _meets(one: Span, two: Span) -> bool:
    region, origin, low, high = one
    other, its, start, end = two
    if not _under(region, other):
        return False
    if region != other or origin != its:
        return True  # a coarser region, or a displacement from another origin
    return low < end and start < high


def _surviving(spans: frozenset[Span], holes: frozenset[Span]) -> list[Span]:
    """The spans no single hole covers.

    One hole, not their union: two exclusions that together cover a span but
    neither of which does alone leave it reachable. That is the conservative
    answer and it is the one `_excluded` already gave.
    """
    return [
        one
        for one in spans
        if not any(
            one[0][: len(hole[0])] == hole[0] and hole[1] == one[1] and hole[2] <= one[2] and one[3] <= hole[3]
            for hole in holes
        )
    ]


EVERYWHERE = RegionSet(frozenset({(ROOT, HERE, *WHOLE)}))


def _absolute(ref, known: dict | None) -> "tuple[tuple[str, ...], str] | None":
    """Axiom 3: a selector the program named as a literal is its own region.

    Asked of the selector's value rather than of the instruction, because by
    the time this matters the load of `b$seg` and the far access through it
    are in different blocks. A singleton interval is a constant.
    """
    if not known or ref.segment is None:
        return None
    interval = known.get(ref.segment)
    if interval is None or interval.low != interval.high:
        return None
    return (*ABSOLUTE, f"{interval.low:#06x}"), HERE


def _region(space, index: int | None, layout=None) -> tuple[tuple[str, ...], str]:
    """The region a space names and the register its displacements count from.

    `index` of None is a reference that knows its kind and not which one --
    the coarser region, which is the point of there being a path at all.
    """
    match space:
        case Space.STACK:
            return STACK, SP
        case Space.FRAME:
            return STACK, BP
        case Space.SEGMENT:
            if index is None:
                return DGROUP, HERE
            owned = isinstance(layout, module.Group) and index not in layout.shared
            return (*(DGROUP if owned else LINKED), f"seg:{index}"), HERE
        case Space.EXTERNAL:
            return LINKED, (HERE if index is None else f"ext:{index}")
        case Space.FAR if index:
            return NAMED, HERE
        case _:
            # LITERAL, FAR and GROUP: a displacement no fixup claims, an
            # address through a segment register, a group index this refuses
            # to resolve. Each could be anywhere, bar the argument below.
            return ROOT, HERE


def _floor(addr) -> frozenset[Span]:
    """What an address is known to miss by virtue of where it is written."""
    if addr is not None and _region(addr.space, addr.index)[0] is ROOT:
        # A displacement in the code reaches anything in DGROUP, and the stack
        # is not in DGROUP. See the region comment above.
        return frozenset({(STACK, SP, *WHOLE)})
    return frozenset()


def _at(addr, width: int, bounds: dict | None, layout=None, indexed: bool = False) -> frozenset[Span]:
    """The bytes an address names, as coarsely as it knows them.

    `indexed` is a reference reached through an index value rather than a
    register its address names: the landmarks bound neither.
    """
    if addr is None:
        return frozenset({(ROOT, HERE, *WHOLE)})
    span = module.reach(addr, max(width, 1), bounds) if bounds and not indexed else None
    if span is None:
        # Indexed with nothing to bound it: every byte of its own region, and
        # none of another's -- axiom 4.
        span = WHOLE if addr.base or indexed else (addr.disp, addr.disp + max(width, 1))
    region, origin = _region(addr.space, addr.index, layout)
    return frozenset({(region, origin, *(WHOLE if region in (ROOT, DGROUP, NAMED) else span))})


def addressed(addr, width: int, bounds: dict | None = None, layout=None) -> RegionSet:
    """Which bytes an address reaches, for a caller that holds no reference."""
    return RegionSet(_at(addr, width, bounds, layout), _floor(addr))


def _holes(ref, layout=None) -> frozenset[Span]:
    """What the reference is known not to reach.

    `beyond` is this -- a call that escapes no pointer into a segment cannot
    reach that segment -- and so is `excludes`, which is why they belong
    together rather than one per code path.
    """
    out: set[Span] = set()
    if ref.beyond is not None:
        owner, reaches = ref.beyond
        if not any(segment == owner for segment, _ in reaches):
            out.add((*_region(Space.SEGMENT, owner, layout), *WHOLE))
    for addr, width in ref.excludes:
        out.add((*_region(addr.space, addr.index, layout), addr.disp, addr.disp + width))
    return frozenset(out) | _floor(ref.addr)


def _spans(ref, bounds: dict | None, known: dict | None = None, layout=None) -> frozenset[Span]:
    """The bytes the reference names, as coarsely as it knows them."""
    within = getattr(ref, "within", None)
    if within:
        # An address the body took of its own locals stays inside them.
        return frozenset((STACK, BP, low, high) for low, high in within)
    absolute = _absolute(ref, known)
    if absolute is not None:
        return frozenset({(*absolute, *WHOLE)})
    if ref.allocation is not None:
        # A $DYNAMIC array is an owning allocation: two descriptors denote two
        # live objects, and B$DDIM's storage is not in DGROUP at all. Which
        # byte of it this is stays for the same-base arithmetic to answer.
        return frozenset({(("alloc", str(ref.allocation)), HERE, *WHOLE)})
    if ref.pointer or ref.addr is None:
        # No byte, but often still a kind: a push is a push whatever the depth.
        return frozenset({(*_region(ref.space, None, layout), *WHOLE)})
    return _at(ref.addr, ref.width, bounds, layout, indexed=ref.base is not None and not ref.addr.base)


def regions(ref, bounds: dict | None = None, known: dict | None = None, layout=None) -> RegionSet:
    """Which bytes this reference may reach.

    `bounds` is the module's own layout -- `module.landmarks` -- and is the
    one input that is not on the reference: an indexed operand reaches its
    whole segment unless the next thing named after it says where it stops.
    `module.reach` is reused rather than restated; it is the same question.
    """
    return RegionSet(_spans(ref, bounds, known, layout), _holes(ref, layout))


def _same_typed_start(one: object, other: object) -> bool:
    """Whether two typed views explicitly start at the same storage.

    This is the union exception to TBAA, not a general must-alias query.  It
    deliberately requires the same SSA address computation (or the same
    concrete canonical slice); two unrelated pointers that happen to compare
    equal at run time still carry C's ordinary strict-aliasing contract.
    """
    one_addr, other_addr = getattr(one, "addr", None), getattr(other, "addr", None)
    one_base, other_base = getattr(one, "base", None), getattr(other, "base", None)
    one_segment, other_segment = getattr(one, "segment", None), getattr(other, "segment", None)
    one_base_width, other_base_width = getattr(one, "base_width", None), getattr(other, "base_width", None)
    if one_addr is not None and other_addr is not None:
        if one_addr.space is Space.FAR and one_base is None and one_segment is None:
            return False
        return (
            one_addr == other_addr
            and one_base == other_base
            and one_segment == other_segment
            and one_base_width == other_base_width
            and getattr(one, "symbolic", None) == getattr(other, "symbolic", None)
            and getattr(one, "allocation", None) == getattr(other, "allocation", None)
        )
    if getattr(one, "pointer", False) and getattr(other, "pointer", False):
        return (
            one_base is not None
            and one_base == other_base
            and one_segment == other_segment
            and one_base_width == other_base_width
        )
    one_provenance = getattr(one, "provenance", None)
    other_provenance = getattr(other, "provenance", None)
    if one_provenance is None or other_provenance is None:
        return False
    if len(one_provenance.slices) != 1 or len(other_provenance.slices) != 1:
        return False
    a, b = next(iter(one_provenance.slices)), next(iter(other_provenance.slices))
    return a.object == b.object and a.low == b.low and a.low != _FLOOR


def typed_apart(one: object, other: object) -> bool:
    """Axiom 5: incompatible non-character scalar lvalues are disjoint.

    GCC and LLVM attach TBAA to indirect loads and stores, not only directly
    named declarations.  Preserve incompatible views of one explicit address
    for C's union rule; every other pair carries the language's no-alias fact.
    """
    a, b = getattr(one, "typed", None), getattr(other, "typed", None)
    return a is not None and b is not None and a[0] != b[0] and not _same_typed_start(one, other)


def may_alias(
    one, other, bounds: dict | None = None, known: dict | None = None, other_known: dict | None = None, layout=None
) -> bool:
    """Whether two references can name the same byte."""
    if typed_apart(one, other):
        return False
    if one.provenance is not None and other.provenance is not None:

        def narrowed(ref, provenance, facts):
            interval = (facts or {}).get(ref.base)
            if (
                ref.base is None
                or ref.addr is None
                or interval is None
                or interval.width != ref.base_width
                or len(provenance.slices) != 1
            ):
                return provenance
            source = next(iter(provenance.slices))
            # Whole-object provenance is the shape an indexed lvalue carries.
            # Its bounded index narrows that object to the bytes this program
            # point can actually touch.
            low = ref.addr.disp + interval.low
            high = ref.addr.disp + interval.high + 1
            end = high + max(ref.width, 1) - 1
            if source.object.extent is not None and not (0 <= low < high and end <= source.object.extent):
                return provenance
            return memory.Provenance(
                frozenset({memory.Slice(source.object, low, high, width=max(ref.width, 1))}),
                provenance.restrict,
            )

        return narrowed(one, one.provenance, known).intersects(narrowed(other, other.provenance, other_known))
    return regions(one, bounds, known, layout).intersects(regions(other, bounds, other_known, layout))


def addresses(a, a_width: int, b, b_width: int, bounds: dict | None = None, layout=None) -> bool:
    """The same question for a caller that holds addresses and no reference."""
    return addressed(a, a_width, bounds, layout).intersects(addressed(b, b_width, bounds, layout))


def provenance(
    ref,
    bounds: dict | None = None,
    known: dict | None = None,
    layout=None,
    private: frozenset[int] = frozenset(),
    spared: frozenset[int] = frozenset(),
) -> memory.Provenance:
    """The region set as objects, for migrating references onto provenance alone.

    `private` are the segments no pointer reaches unless handed out: their
    objects are uncaptured, and a call reaches one only where `beyond` names
    it. `spared` are segments some call is proven to miss part of: named by
    every reference that reaches them, as GCC's ipa-reference does, so that
    the exclusion has an object to be taken from. Holes are subtracted from the objects they fall in. An exclusion
    from a coarse region has no object form, so it widens; see
    `tools/provdiff.py`.
    """
    if ref.symbolic is not None:
        symbol = ref.symbolic
        at = module.Addr(symbol.space, symbol.offset + symbol.addend, symbol.index)
        ref = replace(ref, addr=at, base=None, segment=None)
    spans = _spans(ref, bounds, known, layout)
    root = (ROOT, HERE, *WHOLE)
    if root in spans and ref.beyond is not None:
        spans = spans - {root} | _reached(ref.beyond, layout)
    elif root in spans and not _floor(ref.addr):
        # The push area is unaddressed, so a reference that can reach it names it.
        spans = spans | {(STACK, SP, *WHOLE)}
    framed = any(addr.space is Space.FRAME for addr, _ in ref.excludes)
    if any(one[:2] == (STACK, SP) for one in spans) and not framed:
        # sp and bp displacements are not comparable: a push is anywhere in
        # the frame, unless proven clear of the locals, which puts it in the
        # push area alone.
        spans = spans | {(STACK, BP, *WHOLE)}
    slices = set()
    for region, origin, low, high in spans:
        if region == DGROUP:
            # Some segment of the group: the private ones are named, being uncaptured.
            slices |= {
                memory.Slice(_object((*DGROUP, f"seg:{one}"), HERE, private | spared)) for one in private | spared
            }
        if region == NONLOCAL:
            slices |= {
                memory.Slice(_object((*DGROUP, f"seg:{one}"), HERE, private | spared)) for one in spared - private
            }
        object_ = _object(region, origin, private | spared)
        if (low, high) == WHOLE or object_.kind in (memory.Kind.UNKNOWN, memory.Kind.NONLOCAL):
            slices.add(memory.Slice(object_))
        else:
            slices.add(memory.Slice(object_, low, high))
    for addr, width in ref.excludes:
        hole = _object(*_region(addr.space, addr.index, layout), private | spared)
        slices = {part for one in slices for part in _without(one, hole, addr.disp, addr.disp + width)}
    return memory.Provenance(frozenset(slices))


def _reached(beyond, layout) -> frozenset[Span]:
    """A bounded call's reach, positively: everything but the program's own segment, and that where handed out."""
    owner, reaches = beyond
    out = {(NONLOCAL, HERE, *WHOLE), (STACK, SP, *WHOLE)}
    if any(segment == owner for segment, _ in reaches):
        out.add((*_region(Space.SEGMENT, owner, layout), *WHOLE))
    return frozenset(out)


def _without(one: memory.Slice, hole: memory.Object, low: int, high: int) -> list[memory.Slice]:
    if one.object != hole or one.stride != 1 or high <= one.low or one.high <= low:
        return [one]
    parts = ((one.low, low), (high, one.high))
    return [memory.Slice(one.object, start, end) for start, end in parts if start < end]


def _object(region: tuple[str, ...], origin: str, private: frozenset[int] = frozenset()) -> memory.Object:
    if region == STACK:
        if origin == SP:
            return memory.Object(memory.Kind.STACK, origin, addressed=False, captured=False)
        return memory.Object(memory.Kind.FRAME, origin)
    if region[:1] == ALLOCATION:
        return memory.Object(memory.Kind.ALLOCATION, region[1:])
    if region[:1] == ABSOLUTE and len(region) > 1:
        return memory.Object(memory.Kind.ABSOLUTE, region[1])
    if region == NAMED:
        return memory.Object(memory.Kind.NAMED)
    if region[-1:] and region[-1].startswith("seg:"):
        index = int(region[-1][4:])
        return memory.Object(memory.Kind.GLOBAL, (Space.SEGMENT, index), captured=index not in private)
    if region == LINKED:
        return memory.Object(memory.Kind.EXTERNAL, origin or None)
    if region in (DGROUP, NONLOCAL):
        return memory.Object(memory.Kind.NONLOCAL)
    return memory.Object(memory.Kind.UNKNOWN)
