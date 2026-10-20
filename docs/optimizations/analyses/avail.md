> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/avail.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/avail.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/avail.rs sha256:c27ad68758f04141cd508134472017b01e75a37708946f33ced5ee350fbdf5b3 -->

# avail

Dead stores, and the cells loads and stores name, as LLVM's DSE finds them.

## How it works
The memory model is `MemRef` cells and [`crate::memoryssa`](memoryssa.md)'s `Accesses`.
Load forwarding is `llrm-transforms`' `gvn` (MemoryDependence), not here.

`dead_stores` is a backward must-overwrite analysis over the
CFG. Every cell a function stores to is numbered once (`Stored`), as LLVM's
DSE numbers its memory locations, and a block's state is a set of numbered
cells overwritten before any read. Walking a block backward:

- a plain store whose cell is covered by one overwritten cell, or whose
  bytes the overwritten cells together write (`pieced`), is dead; the cell
  then joins the set;
- an instruction that reads or writes anything else removes the cells it may
  touch (`clobber`, using `may_clobber` and `EscapedBefore` where given),
  asked of the cells `reaching` names: those of its own object or frame
  whose bytes meet its own, by a sorted index of bytes, not every cell of
  the object (`LLRM_CHECK_REACHED` asserts a cell it leaves out is ruled
  out);
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
The first store is overwritten before any read, so `dead_stores`
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
`dead_stores`, `dead_stores_escaping`, and the accessors `loaded_into`,
`stored_from`, `stored_cell`. Consumer: `dse` in `llrm-transforms`.

## Needs
[`crate::memoryssa`](memoryssa.md), [`crate::alias`](alias.md) (`EscapedBefore`), [`crate::ranges`](ranges.md)
(register constants), [`crate::cellmap`](cellmap.md), [`crate::regions`](regions.md) and
[`crate::cfg`](cfg.md).

## See also
LLVM `MemoryDependenceAnalysis`, GVN `processLoad` and
`DeadStoreElimination`.
