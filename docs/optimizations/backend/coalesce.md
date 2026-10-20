> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/coalesce.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/coalesce.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/coalesce.rs sha256:9b88dbf4a372a8abe321815ad3dae9cdadc03adf7f87dfd5150e044e3a72c8a1 -->

# coalesce

Removes a copy whose two values can share a register: the phase that runs
LLVM's `RegisterCoalescer` (`crate::coalescer`), and the interference
graph `twoaddr` and the spiller read.

## How it works
After `phielim` and `twoaddr` the body is full of `mov v2, v1` between
values. If the live intervals of `v1` and `v2` can be joined (the values
they hold agree wherever both are live), they are one value and the move
is an identity that disappears:

```text
 before                          after
   v1 = load ...                   v1 = load ...
   v2 = mov v1       (no clash)    ... v1 ...     v2 renamed v1
   ... v2 ...                      the mov is gone
```

The phase builds the intervals ([`crate::liveness::liveintervals`](liveness.md)) and
hands them to `crate::coalescer` with the target's answers (which values
may share a register: the palettes and the pins). Joining is by interval
and leaves colouring to the allocator, which splits where it must; the old
pass here (Briggs and George over an interference graph, with a call-clobber
check) is gone, objects unchanged on the corpora the day it went. A body
with phis is not coalesced.

What stays in this file is the interference graph: `_interference` (built by
walking each block backwards: values live at the same point are adjacent,
the two ends of a copy not by that copy, and the sources of a parallel copy
before any of its destinations is written) and `_interference_among`, read
by `twoaddr.rs` and by the spiller's sibling pricing; `_merged` and
`_copy`.

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

## See also
LLVM's `RegisterCoalescer`:
<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp>
