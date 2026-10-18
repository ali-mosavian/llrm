> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/memcpyopt.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/memcpyopt.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/memcpyopt.rs sha256:238a2eb295df8d58508645849bfa6f7ba73a606df8f632e4217187f47a7bcb66 -->

# memcpyopt

Loads and stores that copy adjacent bytes become one `llvm.memcpy` (LLVM's
MemCpyOpt, which makes a call of the stores that copy a struct; gcc has no
such pass and leaves them to the by-pieces expansion).

## How it works
In each block, a pair is a non-volatile integer load whose only user is a
non-volatile store of it in the same block, each at a constant offset from a
root pointer. A run is the pairs of one source root and one destination root
that tile adjacent bytes of both, the destination at the same distance from
the source throughout. Pairs are taken in one walk of the block: any other
instruction that touches memory, or a pair of other roots, ends the pairs
open so far, so nothing stands between a run's first load and last store but
work that touches no memory.

The run is one `llvm.memcpy` of its bytes where its last store stood, when
two things hold. The alias owner (`regions::overlapping`, over the
references `Unit::annotated` gives) says no source cell and destination cell
of the run can meet, so the order of its loads and stores does not matter.
And it is cheaper: the pairs' loads and stores against what the same
`length` costs (`loopidioms::copy_price`: the loads and stores isel makes of
a short copy, else `rep movs`). Four byte pairs are one dword pair, which is
a gain; two dword pairs are what isel makes of the copy, which is none.

```text
  %a = load i32, ptr %s            call memcpy(%d, %s, 40)
  store i32 %a, ptr %d      ==>    (nine more pairs gone)
  ... nine more pairs
```

## Example
Ten dword pairs, `d[i] = s[i]`, two globals:

```mir
%a0 = load i32, ptr @s
store i32 %a0, ptr @d
%p1 = getelementptr i32, ptr @s, i32 1
%a1 = load i32, ptr %p1
...
```

becomes `call void @llvm.memcpy.p0.p0.i32(ptr @d, ptr @s, i32 40, i1
false)`.

From unit test `ten_dword_pairs_are_one_memcpy` in `memcpyopt_tests.rs`.

## Runs
After the function fixed point, beside `Merge` (no pass asks a memcpy what
its destination holds, and `splitcopy` would split the memcpy of a local
again). On and off with `-ftree-loop-distribute-patterns`, as `LoopIdioms`.

## Needs
`regions::overlapping` through `Unit::annotated`; the target's costs.

## Keeps
The blocks and edges: only instructions change.

## See also
[`crate::loopidioms`](loopidioms.md) owns `copy_price` and the loop that copies;
[`crate::splitcopy`](splitcopy.md) is the opposite for a local.
