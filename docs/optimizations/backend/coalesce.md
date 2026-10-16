> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/coalesce.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/coalesce.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/coalesce.rs sha256:a0a977536fcaa4c52acfad76699801386b38d62d45e20edb156865a969c470c5 -->

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

## Example
A multiply by 5: `t3` joins `t4`, so `t3 = mov t4` (`#8`) goes and the add
updates `t4`. `t5 = mov t4` stays: `t4` and `t5` are pinned to EAX and EDX
by the `ret`.

Before (`TwoAddress`):

```lir
; ...
     #7  t4         = mov.i32     t2
     #3  t4         = shl.i32     t4, 2:i8
     #8  t3         = mov.i32     t4
     #4  t3         = add.i32     t3, t2
     #9  t5         = mov.i32     t3
     #5  t5         = shr.i32     t5, 16:i8
     #6               ret         t3@eax, t5@edx
```

After (`Coalescer`):

```lir
; ...
     #7  t4         = mov.i32     t2
     #3  t4         = shl.i32     t4, 2:i8
     #4  t4         = add.i32     t4, t2
     #9  t5         = mov.i32     t4
     #5  t5         = shr.i32     t5, 16:i8
     #6               ret         t4@eax, t5@edx
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
