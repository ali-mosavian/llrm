> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/coalesce.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/coalesce.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/coalesce.rs sha256:2b0600f4723dfb7ba248fed08f47df0f2324a5a3cc4df44fd820b56a48a2d6c3 -->

# coalesce

Removes a copy whose two values can share a register.

## How it works
After `phielim` and `twoaddr` the body is full of `mov v2, v1` between
values. If `v1` and `v2` never need different registers at once, they can
be one value and the move is an identity that disappears.

```text
 before                          after
   v1 = load ...                   v1 = load ...
   v2 = mov v1       (no clash)    ... v1 ...     v2 renamed v1
   ... v2 ...                      the mov is gone
```

Each value has a live interval (`analysis::intervals`). Copies are visited
block by block, the deepest loops first and, within a loop, the busiest
blocks first, so that of two joins that clash the one in the hotter code
stays. For one copy `(a, b)` the webs already joined are looked up
(union-find) and the join is refused when:

- they are the same web, or are pinned to different registers;
- their live ranges share a point, unless the copy is of one width, begins
  the only run they share, and neither web is written inside it (both then
  hold the copied value there), and it is not a member of a parallel copy;
- the registers both may take (their class palettes) have none in common, or
  every common register is destroyed somewhere in the merged interval
  (`allocate::_clobbered`), or a pin is outside what is left.

The joined web's interval is the union of the two (`_merged`) and its pin
is the pin of either; the pinned web is the survivor. At the end every
value is renamed to its web's root and identity moves are dropped.

There is no interference graph and no Briggs or George test. Two live
ranges that share no point leave the live count of every point as it was,
so the join makes no point more crowded; what the allocator does with a
range of many segments is its to split and evict. Of 567 copies in
`d_faces` at `-O2` the graph joined 174 and this joins 518, for 15.8 M
of graph and 13.9 M of intervals built before the first copy was looked
at, 55.2 M against 18.5 M for the whole pass.

## LLVM structure, and what is not translated
LLVM's `RegisterCoalescer`
([`RegisterCoalescer.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp))
orders the copies by loop depth, then split edges, then CFG degree
(`copyCoalesceInMBB`, L4058), joins by `joinCopy` (L2054) over
`LiveIntervals` with `JoinVals` (L3154-L3222) giving values that are copies
of each other one number, and retries deferred copies until nothing changes
(L4286-L4321). This follows the order of the worklist (depth, then
frequency) and the overlap test with its same-value exception. It deviates,
each until the `liveness` module's `LiveRange` lands (`regalloc-core`'s
translation of `LiveInterval.cpp`): no `VNInfo`s, so the same-value rule is
the one case `JoinVals` finds by value number (a width-equal copy starting
the only shared run); no `CoalescerPair` classification of physical,
sub-register and virtual joins (pins and palettes stand in); no
`ConflictResolution` (`CR_Keep/Erase/Replace/Merge/Unresolved`), no lane
masks or sub-ranges; no deferral and retry (a refused copy is not tried
again after later joins); no `removeCopyByCommutingDef`,
`reMaterializeTrivialDef` or `LargeIntervalSizeThreshold`.

## Example
A multiply by 5: `v3` joins `v4`, so `v3 := mov v4` goes and the add updates
`v4`. `v5 := mov v4` stays: `v4` and `v5` are pinned to EAX and EDX. `;
inserted` notes are dropped.

Before (`TwoAddress`):

```lir
; ...
0x0001  v4 := mov v2
0x0001  v4 := shl v4, 0x2
0x0001  v3 := mov v4
0x0001  v3 := add v3, v2
0x0002  v5 := mov v3
0x0002  v5 := shr v5, 0x10
0x0002     ; requires v3@EAX,v5@EDX
```

After (`Coalescer`):

```lir
; ...
0x0001  v4 := mov v2
0x0001  v4 := shl v4, 0x2
0x0001  v4 := add v4, v2
0x0002  v5 := mov v4
0x0002  v5 := shr v5, 0x10
0x0002     ; requires v4@EAX,v5@EDX
```

From `ISEL_DUMP=1 llrm-qb -O2 -march=i386` of `Times5& = x * 5` (function
`TIMES5&`).

## Runs
Once per function, in the machine phases after `twoaddr` and before
`regalloc`, at every -O level. Both routes run it.

## Needs
[`crate::backend::classes`](classes.md) for the palettes, [`crate::backend::regclass`](regclass.md)
for each value's register class, `allocate::_masks` for the points that
destroy registers, and `analysis::intervals` for the live ranges.
`_interference` and `_interference_among` stay for `twoaddr.rs` and the
spiller's sibling pricing.
