> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/avail.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/avail.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/avail.rs sha256:ed2b93783f1b8dadd2d03a93335b3d132141c605c7a6c1b13988431af424a408 -->

# avail

Available loads and dead stores, as LLVM's GVN load forwarding and DSE find
them through MemoryDependence.

## How it works
Two independent queries share the memory model (`MemRef` cells and
[`crate::memoryssa`](memoryssa.md)'s `Accesses`).

Forwarding. `forwardable_walk` takes a set of loads and, for each plain
(non-volatile) load, asks MemorySSA for the nearest writes that may clobber
its bytes and tries three sources in order:

1. One clobber that is a plain store to the same bytes, defined before the
   load, whose value has the load's type: the load is that value.
2. Two or more clobbers (at most `MAX_NEAREST_WRITES`) that are all exact
   stores of one value defined where the load can read it, such as the arms
   of a join that each stored the same thing.
3. A dominating load of the same bytes, nearest first, with nothing between
   the two that may change them (`MemorySSA::unchanged`). A candidate whose
   walk fails also fails every load that dominates it.

A value serves a read only of its own type. Each answer is a `Forward { at,
value }`.

```text
   store i32 %v, p          %a = load p     <- 1: %a is %v
   ...                      ...
   %a = load p              %b = load p     <- 3: %b is %a
   ...
   br c, T, F      T: store %v, p   F: store %v, p
   J: %b = load p                                 <- 2: %b is %v
```

Dead stores. `dead_stores` is a backward must-overwrite analysis over the
CFG. Every cell a function stores to is numbered once (`Stored`), as LLVM's
DSE numbers its memory locations, and a block's state is a set of numbered
cells overwritten before any read. Walking a block backward:

- a plain store whose cell is covered by one overwritten cell, or whose
  bytes the overwritten cells together write (`pieced`), is dead; the cell
  then joins the set;
- an instruction that reads or writes anything else removes the cells it may
  touch (`clobber`, using `may_clobber` and `EscapedBefore` where given),
  asked of the cells `reaching` names: those of its own object or frame
  whose bytes meet its own, by a sorted index of bytes where the pick of
  buckets would visit more than `DENSE_VISITS` cells (the index is built by
  the first such query), by the cells of the pick otherwise
  (`LLRM_CHECK_REACHED` asserts a cell it leaves out is ruled out);
- a fill (what a call writes before it reads) adds its cells.

At a join the state is the intersection over the successors, computed per
cell with "some cell naming the same bytes" (`alike`) so a spelling
difference does not lose it. The fixed point starts from every stored cell
and shrinks, with a worklist in postorder, and only the last verdicts stand.
A block with no successor starts from nothing, since the caller may read the
cell; with `private` (a predicate for cells nothing outside the function can
read) it starts from the private cells; a block ending in `unreachable`
starts from every cell. A call whose accesses are not known keeps only the
private cells in the set (nothing but a named access can read those); any
other unknown instruction empties it.

Cells that share bytes share an overlap bucket (`regions`), so a write asks
its bucket and not all cells; a cell is related to another (covers, names
the same bytes, is placed against) only where `placed` can place them, which
takes one displaced frame or one base pointer and bytes that meet, so it is
compared with those (`Members::meeting`) and not with its object's cells.

## Example
Forwarding. Both arms of a join store `%v` to the cell, so the load at the
join is `%v` (with `%w` on one arm instead, it is not forwarded):

```mir
define i16 @f(i1 %c, i16 %v, i16 %w) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 %v, ptr getelementptr (i8, ptr @g, i16 32)
  br label %b3

b2:
  store i16 %v, ptr getelementptr (i8, ptr @g, i16 32)
  br label %b3

b3:
  %x = load i16, ptr getelementptr (i8, ptr @g, i16 32)
  ret i16 %x
}
```

`forwardable_walk` returns `Forward { at: the load in b3, value: %v }`.

From unit test `test_a_join_keeps_what_every_predecessor_agrees_on` in
`avail_tests.rs`.

Dead store. The first store is overwritten before any read, so `dead_stores`
returns it:

```mir
define void @f(ptr %p) {
b0:
  store i16 1, ptr getelementptr (i8, ptr @g, i16 32)
  store i16 2, ptr getelementptr (i8, ptr @g, i16 32)
  ret void
}
```

From unit test `test_a_store_is_dead_only_when_overwritten_before_any_read`
in `avail_tests.rs`.

## Provides
`forwardable_walk`, `dead_stores`, `dead_stores_escaping`, and the
accessors `loaded_into`, `stored_from`, `stored_cell`. Consumers:
`transform` (load forwarding), `loadjoins` and `dse` in `llrm-transforms`.

## Needs
[`crate::memoryssa`](memoryssa.md), [`crate::alias`](alias.md) (`EscapedBefore`), [`crate::ranges`](ranges.md)
(register constants), [`crate::cellmap`](cellmap.md), [`crate::regions`](regions.md) and
[`crate::cfg`](cfg.md).

## See also
LLVM `MemoryDependenceAnalysis`, GVN `processLoad` and
`DeadStoreElimination`.
