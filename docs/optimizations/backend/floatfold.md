> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/floatfold.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/floatfold.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/floatfold.rs sha256:65338dfd9205ba5609dce14e55e9fac7df2628405f8ab48e20ea97d84b1b2487 -->

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

## Example
`p.x = p.x * k + p.y; return p.x - k`: each load of a field is read once, so
it becomes the memory operand of the arithmetic that reads it. The `fsub`
had the loaded cell on its left, so it is the reversed `fsubr`.

Before (after `PressureSink`):

```lir
mix: 11 instructions
  block 0x0000
    0x0000     ; inserted; delivers v1@EAX
    0x0000  v2 := fld [[bp+0x8]]  ; inserted
    0x0000  v3 := fld [None v1@unplaced]  ; inserted
    0x0001  v4 := fmul v3, v2  ; inserted
    0x0003  v5 := fld [None v1@unplaced]  ; inserted
    0x0004  v6 := fadd v4, v5  ; inserted
    0x0005  [None v1@unplaced] := fstp v6  ; inserted
    0x0006  v7 := fld [None v1@unplaced]  ; inserted
    0x0007  v8 := fsub v7, v2  ; inserted
    0x0008   v8  ; inserted
    0x0008     ; inserted
```

After (`FloatFold`):

```lir
mix: 8 instructions
  block 0x0000
    0x0000     ; inserted; delivers v1@EAX
    0x0000  v2 := fld [[bp+0x8]]  ; inserted
    0x0001  v4 := fmul v2, [None v1@unplaced]  ; inserted
    0x0004  v6 := fadd v4, [None v1@unplaced]  ; inserted
    0x0005  [None v1@unplaced] := fstp v6  ; inserted
    0x0007  v8 := fsubr v2, [None v1@unplaced]  ; inserted
    0x0008   v8  ; inserted
    0x0008     ; inserted
```

From `ISEL_DUMP=1` of the nib function `mix(p: &mut P, k: f64)` with `-m32
-O0`.

## Runs
Machine phase after `pressuresink` and before `floatassign`, once per
function. Not gated by an -O level.

## Needs
[`crate::backend::storedhomes`](storedhomes.md) for the may-write test; helpers from
[`crate::backend::floatalloc`](floatalloc.md).

## See also
`floatassign::fused`; LLVM `X86InstrInfo::foldMemoryOperand`.
