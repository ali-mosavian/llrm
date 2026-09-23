"""Machine-independent memory objects and byte-accurate access paths.

This is the canonical answer to "what storage can this reference name?".
Frontends may discover the answer in different ways, but optimisation sees
only objects and slices.  Slices are half-open; a stride greater than one
describes byte lanes selected by an indexed access, not merely its hull.
"""

from math import gcd
from typing import NamedTuple
from functools import cache
from enum import StrEnum
from dataclasses import field
from dataclasses import dataclass


class Kind(StrEnum):
    UNKNOWN = "unknown"
    # The outgoing push area: only pushes, pops and calls name it.
    STACK = "stack"
    FRAME = "frame"
    GLOBAL = "global"
    EXTERNAL = "external"
    # Storage reachable without an argument: globals, heap and anything
    # whose address escaped this activation. It deliberately excludes an
    # unescaped current frame object.
    NONLOCAL = "nonlocal"
    ALLOCATION = "allocation"
    ABSOLUTE = "absolute"
    NAMED = "named"
    PARAMETER = "parameter"


@dataclass(frozen=True, slots=True)
class Object:
    kind: Kind
    identity: object | None = None
    generation: int = 0
    extent: int | None = None
    # Facts about the object, not its identity: two spellings of one object
    # are the same object whatever they say. LLVM's split, stated once:
    # `addressed` -- some code computes its address, so a pointer of unknown
    # origin may hold it. `captured` -- that address can be found from
    # outside this activation (memory, a return, a callee that keeps it), so
    # NONLOCAL and PARAMETER may reach it. Unaddressed implies uncaptured.
    addressed: bool = field(default=True, compare=False)
    captured: bool = field(default=True, compare=False)


@dataclass(frozen=True, slots=True)
class Slice:
    object: Object
    low: int = -(1 << 31)
    high: int = 1 << 31
    stride: int = 1
    # Consecutive bytes at each selected offset. Most canonical slices use
    # stride 1/width 1; an indexed word array can use stride 4/width 2.
    width: int = 1

    def __post_init__(self) -> None:
        if self.high <= self.low:
            raise ValueError("an alias slice must contain at least one byte")
        if self.stride <= 0:
            raise ValueError("an alias stride must be positive")
        if self.width <= 0:
            raise ValueError("an alias element width must be positive")

    def shifted(self, amount: int) -> "Slice":
        return Slice(self.object, self.low + amount, self.high + amount, self.stride, self.width)

    def intersects(self, other: "Slice") -> bool:
        if not objects_may_alias(self.object, other.object):
            return False
        # Byte offsets have a common origin only for the same concrete object.
        if self.object != other.object:
            return True
        if self.low >= other.high + other.width - 1 or other.low >= self.high + self.width - 1:
            return False
        divisor = gcd(self.stride, other.stride)
        for mine in range(self.width):
            for theirs in range(other.width):
                if (self.low + mine - other.low - theirs) % divisor:
                    continue
                low = max(self.low + mine, other.low + theirs)
                high = min(self.high + mine, other.high + theirs)
                at = self.low + mine + max(0, (low - self.low - mine + self.stride - 1) // self.stride) * self.stride
                limit = min(high, at + other.stride // divisor * self.stride)
                while at < limit:
                    if (at - other.low - theirs) % other.stride == 0:
                        return True
                    at += self.stride
        return False


@dataclass(frozen=True, slots=True)
class Provenance:
    slices: frozenset[Slice]
    # Restricted pointer roots on which the address is based.
    restrict: frozenset[object] = frozenset()

    @classmethod
    def one(
        cls,
        object_: Object,
        low: int = -(1 << 31),
        high: int = 1 << 31,
        *,
        stride: int = 1,
        width: int = 1,
        restrict: object | frozenset[object] | None = None,
    ) -> "Provenance":
        roots = (
            frozenset() if restrict is None else restrict if isinstance(restrict, frozenset) else frozenset({restrict})
        )
        return cls(frozenset({Slice(object_, low, high, stride, width)}), roots)

    def shifted(self, amount: int) -> "Provenance":
        def shifted(one: Slice) -> Slice:
            # A whole object is the top element for offsets within that
            # object.  Repeatedly adding a loop stride to it must remain top:
            # otherwise a pointer recurrence creates a new spelling of the
            # same conservative fact on every dataflow round.
            whole = one.low == -(1 << 31) and one.high == 1 << 31
            bounded_whole = one.object.extent is not None and one.low == 0 and one.high == one.object.extent
            return one if whole or bounded_whole else one.shifted(amount)

        return Provenance(frozenset(shifted(one) for one in self.slices), self.restrict)

    def union(self, other: "Provenance") -> "Provenance":
        return Provenance(self.slices | other.slices, self.restrict | other.restrict)

    def intersects(self, other: "Provenance") -> bool:
        if self.restrict and other.restrict and self.restrict.isdisjoint(other.restrict):
            return False
        return any(one.intersects(two) for one in self.slices for two in other.slices)


class AliasClass(NamedTuple):
    """All `objects_may_alias` asks of an object besides its identity."""

    addressed: bool
    kind: Kind
    captured: bool


def alias_class(one: Object) -> AliasClass:
    return AliasClass(one.addressed, one.kind, one.captured)


def objects_may_alias(one: Object, other: Object) -> bool:
    if one == other:
        return True
    # classes_may_alias's first rule, asked before building either class.
    return one.addressed and other.addressed and classes_may_alias(alias_class(one), alias_class(other))


@cache
def classes_may_alias(one: AliasClass, other: AliasClass) -> bool:
    """Whether two distinct objects of these classes may alias."""
    # Only a reference naming an unaddressed object reaches it.
    if not (one.addressed and other.addressed):
        return False
    for this, that in ((one, other), (other, one)):
        if this.kind is Kind.UNKNOWN:
            return True
    for this, that in ((one, other), (other, one)):
        if this.kind is Kind.NONLOCAL:
            return that.captured and that.kind not in (Kind.FRAME, Kind.STACK)
    for this, that in ((one, other), (other, one)):
        if this.kind is Kind.PARAMETER:
            # An incoming pointer predates this activation and cannot designate
            # one of its frame objects. At a call site the parameter object is
            # replaced by the actual provenance before caller-side queries.
            return that.captured and that.kind is not Kind.FRAME
    if {one.kind, other.kind} <= {Kind.GLOBAL, Kind.EXTERNAL}:
        return Kind.EXTERNAL in (one.kind, other.kind)
    return False
