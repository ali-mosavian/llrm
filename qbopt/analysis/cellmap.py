"""Facts keyed by memory cell, indexed by the object each cell lies in."""

from collections.abc import Callable, Hashable, Iterable

# The bucket of a cell whose object is not known. Every write tests it.
OPEN = object()


class CellMap(dict):
    """A cell-to-fact dict that also buckets its cells by object.

    A write can only reach cells in objects it may alias, so `kill` tests
    those buckets and never the rest. Scanning every cell for every store
    ran deedlines' alias walk through 43M overlap tests in 90 s, all but
    1218 of them between different objects.

    `bucket_of` names a cell's bucket; it must not change while the cell is
    held.
    """

    __slots__ = ("bucket_of", "buckets")

    def __init__(self, bucket_of: Callable[[Hashable], Hashable], items: Iterable = ()) -> None:
        super().__init__()
        self.bucket_of = bucket_of
        self.buckets: dict[Hashable, set] = {}
        self.update(items)

    def __setitem__(self, key, value) -> None:
        if key not in self:
            self.buckets.setdefault(self.bucket_of(key), set()).add(key)
        super().__setitem__(key, value)

    def __delitem__(self, key) -> None:
        super().__delitem__(key)
        bucket = self.bucket_of(key)
        keys = self.buckets[bucket]
        keys.discard(key)
        if not keys:
            del self.buckets[bucket]

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
        new.buckets = {bucket: set(keys) for bucket, keys in self.buckets.items()}
        return new

    def kill(self, reached: Iterable[Hashable] | None, overlaps: Callable[[Hashable], bool]) -> None:
        """Delete every cell a write reaches.

        `reached` are the buckets the write may touch (every bucket when
        None); `overlaps` is the exact test, asked only of their cells.
        """
        buckets = self.buckets.keys() if reached is None else reached
        doomed = [key for bucket in buckets for key in self.buckets.get(bucket, ()) if overlaps(key)]
        for key in doomed:
            del self[key]

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

    def __ior__(self, other):
        self.update(other)
        return self
