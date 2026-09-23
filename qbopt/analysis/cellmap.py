"""Facts keyed by memory cell, indexed by the object each cell lies in."""

import bisect
import itertools
from collections.abc import Callable, Hashable, Iterable


class _Spans:
    """One bucket's cells by the displacement they start at.

    (low, high, cell), sorted by low once `sort` has run since the last
    out-of-order `add`. `widest` is the widest cell ever held, a bound on
    how far below a write a cell meeting it can start; it does not shrink
    when that cell goes.
    """

    __slots__ = ("cells", "sorted", "widest")

    def __init__(self) -> None:
        self.cells: list[tuple[int, int, Hashable]] = []
        self.sorted = True
        self.widest = 0

    def add(self, key, low: int, high: int) -> None:
        self.sorted = self.sorted and (not self.cells or self.cells[-1][0] <= low)
        self.cells.append((low, high, key))
        self.widest = max(self.widest, high - low)

    def sort(self) -> None:
        if not self.sorted:
            self.cells.sort(key=_low)
            self.sorted = True

    def remove(self, key, low: int) -> None:
        at = bisect.bisect_left(self.cells, low, key=_low) if self.sorted else 0
        while not (self.cells[at][0] == low and self.cells[at][2] == key):
            at += 1
        del self.cells[at]

    def meeting(self, low: int, high: int) -> list:
        """The cells whose bytes meet [low, high); `sort` has run."""
        found = []
        for start, end, key in itertools.islice(
            self.cells, bisect.bisect_left(self.cells, low - self.widest + 1, key=_low), None
        ):
            if start >= high:
                break
            if low < end:
                found.append(key)
        return found

    def copy(self) -> "_Spans":
        new = _Spans.__new__(_Spans)
        new.cells = list(self.cells)
        new.sorted = self.sorted
        new.widest = self.widest
        return new


def _low(cell: tuple) -> int:
    return cell[0]


class CellMap(dict):
    """A cell-to-fact dict that also buckets its cells by object.

    A write can only reach cells in objects it may alias, so `kill` tests
    those buckets and never the rest. Scanning every cell for every store
    ran deedlines' alias walk through 43M overlap tests in 90 s, all but
    1218 of them between different objects.

    `bucket_of` names a cell's bucket, a tuple; it must not change while
    the cell is held. `parts[i]` finds buckets by their i-th component, so a
    write can look its buckets up rather than test each one. `span_of`, if
    given, names the bytes [low, high) a cell covers, or None; a bucket's
    cells with bytes are indexed by them, for `kill`'s `displaced`.
    """

    __slots__ = ("bucket_of", "span_of", "buckets", "parts", "spans")

    def __init__(
        self,
        bucket_of: Callable[[Hashable], Hashable],
        items: Iterable = (),
        span_of: Callable[[Hashable], tuple[int, int] | None] | None = None,
    ) -> None:
        super().__init__()
        self.bucket_of = bucket_of
        self.span_of = span_of
        self.buckets: dict[tuple, set] = {}
        self.parts: list[dict[Hashable, set]] = []
        self.spans: dict[tuple, _Spans] = {}
        self.update(items)

    def __setitem__(self, key, value) -> None:
        if key not in self:
            bucket = self.bucket_of(key)
            keys = self.buckets.get(bucket)
            if keys is None:
                keys = self.buckets[bucket] = set()
                for i, part in enumerate(bucket):
                    if i == len(self.parts):
                        self.parts.append({})
                    self.parts[i].setdefault(part, set()).add(bucket)
            keys.add(key)
            if self.span_of is not None and (span := self.span_of(key)) is not None:
                spans = self.spans.get(bucket)
                if spans is None:
                    spans = self.spans[bucket] = _Spans()
                spans.add(key, *span)
        super().__setitem__(key, value)

    def __delitem__(self, key) -> None:
        super().__delitem__(key)
        bucket = self.bucket_of(key)
        keys = self.buckets[bucket]
        keys.discard(key)
        if self.span_of is not None and (span := self.span_of(key)) is not None:
            self.spans[bucket].remove(key, span[0])
        if not keys:
            del self.buckets[bucket]
            self.spans.pop(bucket, None)
            for i, part in enumerate(bucket):
                held = self.parts[i][part]
                held.discard(bucket)
                if not held:
                    del self.parts[i][part]

    def update(self, items=(), **named) -> None:
        pairs = items.items() if isinstance(items, dict) else items
        for key, value in pairs:
            self[key] = value
        for key, value in named.items():
            self[key] = value

    def copy(self) -> "CellMap":
        new = CellMap.__new__(CellMap)
        dict.update(new, self)
        new.bucket_of = self.bucket_of
        new.span_of = self.span_of
        new.buckets = {bucket: set(keys) for bucket, keys in self.buckets.items()}
        new.parts = [{part: set(held) for part, held in parts.items()} for parts in self.parts]
        new.spans = {bucket: spans.copy() for bucket, spans in self.spans.items()}
        return new

    def kill(
        self,
        reached: Iterable[Hashable] | None,
        overlaps: Callable[[Hashable], bool],
        displaced: tuple[set, int, int] | None = None,
    ) -> None:
        """Delete every cell a write reaches.

        `reached` are the buckets the write may touch (every bucket when
        None); `overlaps` is the exact test, asked only of their cells.
        `displaced` is (buckets, low, high): a cell in those buckets can
        only be reached if its bytes meet [low, high), so only those are asked.
        """
        buckets = self.buckets.keys() if reached is None else reached
        doomed = [key for bucket in buckets for key in self._asked(bucket, displaced) if overlaps(key)]
        for key in doomed:
            del self[key]

    def _asked(self, bucket, displaced: tuple[set, int, int] | None) -> Iterable:
        if displaced is not None and bucket in displaced[0] and (spans := self.spans.get(bucket)) is not None:
            spans.sort()
            return spans.meeting(displaced[1], displaced[2])
        return self.buckets.get(bucket, ())

    def setdefault(self, key, default=None):
        if key not in self:
            self[key] = default
        return self[key]

    def pop(self, key, *default):
        if key in self:
            value = self[key]
            del self[key]
            return value
        return dict.pop(self, key, *default)

    def popitem(self):
        raise TypeError("CellMap does not support popitem")

    def clear(self) -> None:
        super().clear()
        self.buckets.clear()
        self.parts.clear()
        self.spans.clear()

    def __ior__(self, other):
        self.update(other)
        return self
