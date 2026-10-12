> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/assumptions.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/assumptions.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/assumptions.rs sha256:b1276cb7b089da7cba3cd896bc80daf6e8080e078ea8561207494317951f0aa6 -->

# assumptions

What each block assumes, as LLVM's AssumptionCache: the conditions of the
`llvm.assume` calls in it.

## How it works
`Assumptions::of` walks the body once and records, per block, the first
operand of every call to the `Assume` intrinsic, in program order.

`here(at)` returns what block `at` itself assumes. `above(shape, at)` climbs
the immediate-dominator chain from `at` and returns what each strict
dominator assumes, nearest block first and the last assume of a block
first.

```text
 b0: assume(c0)            above(b3) = [c1, c0]
  |
 b1: assume(c1)            here(b3)  = [c3]   (not in above(b3))
  |
 b3: assume(c3); use
```

A block's own assumes are not part of `above` for that block, so a use
before an assume in the same block is not covered by it. The caller reads
`here` for the part of the block it is in. `above` returns early when no
block assumes anything.

## Example
The assume in `entry` holds in every block `entry` dominates:

```mir
declare void @llvm.assume(i1)

define i16 @f(i16 %n, i16 %len, i1 %c) {
entry:
  %fits = icmp ult i16 %n, %len
  call void @llvm.assume(i1 %fits)
  br i1 %c, label %loop, label %done
loop:
  br label %done
done:
  ret i16 0
}
```

`here(entry)` is `[%fits]`, and `above` of `loop` and of `done` is
`[%fits]`. `guards::holds` then proves `n <u len` in both blocks.

From unit test `test_an_assume_proves_its_compare_below_its_block` in
`guards_tests.rs`.

## Provides
The assumed conditions per block. Consumers: [`crate::guards`](guards.md) turns them
into `Guard`s, and [`crate::ranges`](ranges.md) narrows value ranges with them. The
manager entry is [`crate::manager::AssumptionCache`](manager.md); a `Unit` without one
builds its own on first ask, which `built()` counts for tests.

## Needs
[`crate::cfg::Shape`](cfg.md) for the dominator tree.

## See also
LLVM `AssumptionCache`.
