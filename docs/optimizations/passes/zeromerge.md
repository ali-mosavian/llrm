> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/zeromerge.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/zeromerge.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/zeromerge.rs sha256:3007e68b6e26fdaacdb3837a6015987c54c96de098dc25aaf7cbd08bc9bb05fd -->

# zeromerge

One zero fill for the zeroed locals that follow each other in the entry
block. llrm only.

## How it works
In the entry block, a run of `llvm.memset(%a, 0, n)` calls (nothing
between them), each over a whole static `alloca` of `n` bytes, becomes one
`alloca` of the sum, one `getelementptr` per member and one memset. Each
member sits at its own alignment inside the new `alloca`.

The members are the memsets that loop idioms make from the zero-fill loops
of Nib and QB locals. SROA has run by now, so nothing splits the block
again.

| # | DEVIATE |
|---|---|
| D1 | The pin never joins allocas (`MemCpyOpt` merges ranges of one object). In real mode every member of 64 bytes or more is a `rep stos` fill of 9 bytes of setup (`lea di`, `mov cx`, `rep stosw`); the merged one pays it once. bench/matmul.nib -Os: three fills of a, b and c, -14 B. Owner m16; exit 2026-11-20: the members are one object in the frontend. |

## Example
Before:

```mir
%a = alloca [128 x i8]
%b = alloca [128 x i8]
call void @llvm.memset.p0.i16(ptr %b, i8 0, i16 128, i1 false)
call void @llvm.memset.p0.i16(ptr %a, i8 0, i16 128, i1 false)
```

After:

```mir
%z = alloca [256 x i8]
%a = getelementptr inbounds i8, ptr %z, i16 128
call void @llvm.memset.p0.i16(ptr %z, i8 0, i16 256, i1 false)
```

From unit test `test_zeroed_locals_in_a_row_share_one_fill` in
`zeromerge_tests.rs`.

## Runs
Once per body, after `extload`, with `wanted("zeromerge")`: on where a
pointer is 16 bits.

## Needs
Nothing.

## Keeps
Blocks, dominators and loops.

## See also
Tests: `zeromerge_tests.rs`.
