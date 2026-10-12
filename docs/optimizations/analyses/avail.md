> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/avail.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/avail.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/avail.rs sha256:a7cbbb1e7572b421c74e22c9dec65eda10cd3570e0f4090a38a7f505c8d76da0 -->

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
  touch (`clobber`, using `may_clobber` and `EscapedBefore` where given);
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

Cells that share bytes share an overlap bucket (`regions`), so a cell is
compared with its bucket and not with all cells.

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
