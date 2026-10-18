> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/coalesce.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/coalesce.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/coalesce.rs sha256:2d3372636de8ecaab2d4d3017b29ce827669a5ac3bc4c2aaab7f39768965e5f5 -->

# coalesce

Removes a copy whose two values can share a register.

## How it works
After `phielim` and `twoaddr` the body is full of `mov v2, v1` between
values. If `v1` and `v2` never need different registers at once, they can
be one value and the move is an identity that disappears. This is
conservative coalescing: a join cannot be undone, so it is made only
where it provably cannot make the body harder to colour.

```text
 before                          after
   v1 = load ...                   v1 = load ...
   v2 = mov v1       (no clash)    ... v1 ...     v2 renamed v1
   ... v2 ...                      the mov is gone
```

The values are the nodes of an interference graph (`_interference`, built
by walking each block backwards: values live at the same point are
adjacent, the two ends of a copy not by that copy, and the sources of a
parallel copy before any of its destinations is written). Each value also
has a live interval. Copies are visited block by block, the deepest loops
first and, within a loop, the busiest blocks first, so that of two joins
that clash the one in the hotter code stays. For one copy `(a, b)` the webs
already joined are looked up (union-find) and the join is refused when:

- they are the same web, or are pinned to different registers;
- they are adjacent in the graph;
- the registers both may take (their class palettes) have none in common, or
  every common register is destroyed somewhere in the merged interval
  (`allocate::_clobbered`), or a pin is outside what is left;
- Briggs and George both fail.

Briggs counts the neighbours of the joined web that are significant (their
degree reaches the size of their palette); the join is safe when fewer
than `k` of them, `k` being the registers left. If that fails, George
accepts when `gone` can join `kept` (or the reverse) because each neighbour
of `gone` is already a neighbour of `kept`, cannot take a register of the
join, or is neither pinned nor significant. George is not tried if either
web is pinned.

`Webs` keeps the significant and pinned neighbour counts per web as webs
join, so a join costs the smaller web's neighbours, not the larger's. The
joined web's interval is the union of the two (`_merged`), its pin is the
pin of either, and the web kept is the one with more neighbours. At the
end every value is renamed to its web's root and identity moves are
dropped. `LLRM_CHECK_COALESCE=1` checks each count against a scan.

`LLRM_COALESCE_LLVM=1` runs the translation of LLVM's `RegisterCoalescer`
(`crate::coalescer`) in place of all of the above; `LLRM_SHADOW_COALESCE=1`
runs it beside, and prints the copies each leaves. It joins by interval
overlap and leaves colouring to the allocator, so it joins more (bench
`-O2` m32: 939 copies left of 5,499, against 2,342 here) and `d_faces` is
2.5% smaller; `fib`, `frames`, `queens` and `rectwo` grow (+128, +128, +32,
+32 bytes), which is why it is not the default.

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
for each value's register class, and `allocate::_masks` for the points
that destroy registers. `_interference` and `_interference_among` are also
read by `twoaddr.rs` and by the spiller's sibling pricing.

## See also
LLVM's `RegisterCoalescer` joins by live range and keeps no graph:
<https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/RegisterCoalescer.cpp>

The two tests: Briggs, Cooper and Torczon, "Improvements to graph coloring
register allocation" (1994), and George and Appel, "Iterated register
coalescing" (1996).
