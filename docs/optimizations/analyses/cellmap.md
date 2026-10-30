> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/cellmap.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/cellmap.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/cellmap.rs sha256:33e39c0b185d4fd0cfc1386425dc5b666b34c8c6f624deafe4a09d96a4d25ddd -->

# cellmap

A map from memory cell to fact, indexed by the object each cell lies in, so
a write is tested only against the cells it can reach.

## How it works
`CellMap<K, V, B>` keeps `K -> V` items in insertion order and groups the
keys into buckets `B` (a `Bucket`: the object, or the object's part, a cell
lies in). The caller hands `insert` each new cell's bucket and, if it has
one, its byte span; the map never recomputes them, so a cell's bucket must
not change while the cell is held. Cells with a span are also kept sorted
by start, per bucket (`Spans`), with the widest span ever held as a bound
on how far below a write an overlapping cell can start.

```text
 CellMap
  items    k0 -> v0, k1 -> v1, k2 -> v2          (insertion order)
  buckets  objA: {k0, k2}   objB: {k1}
  spans    objA: [(low=0,k0) (low=8,k2)]         sorted by start
  parts    bucket index by component (`Bucket::held` / `released`)

 kill(reached = [objA], overlaps, displaced = (objA, [4,12))):
   asks `overlaps` only of cells in objA whose bytes meet [4,12)
```

`kill(reached, overlaps, displaced)` deletes every cell a write may reach.
`reached` names the buckets to look in (all when `None`); `overlaps` is the
exact test, asked only of the cells the index returns; `displaced` limits
those further, in the buckets it names, to cells whose span meets its span.
`asked` returns the same candidates without deleting. The index may only
skip work: under test a flag makes `kill` ask the exact test of every cell
it skipped.

## Example
No honest example: `CellMap` is a container with no printed form, and no
test shows it alone. Read the tests of its users, such as `consts_tests.rs`.

## Provides
The container. Consumers: [`crate::alias`](alias.md), [`crate::consts`](consts.md),
[`crate::avail`](avail.md), [`crate::regions`](regions.md) and `promote` in `llrm-transforms`.

## Needs
[`crate::regions::ByteRange`](regions.md).
