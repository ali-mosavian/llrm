> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/floatfold.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/floatfold.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/floatfold.rs sha256:605edb305b6ffa02fb9a81ad4786ce3888a498dbca722c70f994206208489345 -->

# floatfold

A float load that only one arithmetic instruction reads becomes that
instruction's memory operand: `fld [m]; faddp` is `fadd [m]`.

## How it works
x87 arithmetic has a memory form for a float or a double (`fadd qword ptr
[m]`). Folding saves the load and the x87 register the loaded value held.
LLVM folds a single-use load the same way
(`X86InstrInfo::foldMemoryOperand`). `floatassign` folds only the reload of
a spilled value; this folds any load, before values are placed.

For each two-operand `FloatArith` on two different values, the right operand
is tried first (the left stays on the stack top, as `fadd st, [m]` has it),
then the left. An operand qualifies when:

- it is read exactly once in the body;
- its defining instruction is the nearest earlier definition in the block, a
  plain `fld` or `fild` of a memory cell, not volatile, with no clobbers and
  not part of a group;
- nothing between the load and the arithmetic is volatile, a call, a barrier
  or an escape, or may write the cell (`storedhomes::may_write`);
- the cell is 4 or 8 bytes. An integer load stays: `fild` then `fadd` is
  faster than `fiadd` on a 486.

Where the loaded cell is the left operand the operation is reversed (`fsub`
becomes `fsubr`, `fdiv` becomes `fdivr`). The arithmetic then reads the
other value and the cell, and the load is deleted.

```text
 before                          after
 v1 = fld  [m]                   v2 = fadd v0, [m]
 ...  (nothing writes [m])       ...
 v2 = fadd v0, v1
```

## Runs
Machine phase after `pressuresink` and before `floatassign`, once per
function. Not gated by an -O level.

## Needs
[`crate::backend::storedhomes`](storedhomes.md) for the may-write test; helpers from
[`crate::backend::floatalloc`](floatalloc.md).

## See also
`floatassign::fused`; LLVM `X86InstrInfo::foldMemoryOperand`.
