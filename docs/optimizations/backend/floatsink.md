> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/floatsink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/floatsink.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/floatsink.rs sha256:34e040734c9d433c65a580ee9c1022c13579fd0adeb16ebe79cfd956a1447d1d -->

# floatsink

A float load moves down to the instruction that reads it.

## How it works
`fld a; fld b; fmul a, K; fadd _, b` leaves `a` below `b`, so the multiply
needs `fxch` to reach `a`. The load of `b` can wait until the add: `fld a;
fmul K; fld b; faddp` has no exchange. A load whose only reader is later in
its block, with a float operation between, is moved to just before that
reader, past nothing that may write its cell.

The load stays where it is when

- it is volatile, clobbers, is part of a group, or is read more than once or
  outside its block;
- between it and the reader there is a call, a barrier, an escape, a
  volatile access, a write to the cell, or a definition of what the load's
  address reads;
- nothing between is float arithmetic (nothing to exchange around);
- the target's `x87_exchange` price is under 2 clocks (P5 and later pair
  it), or the module is BASIC (a signalling NaN would fault elsewhere).

DEVIATE SD-SINK: the pin has no such step; clang's `fld; fld; fxch; fmul`
is the same. Reason: 4 clocks per exchange on the 486 (x_ld_axpy -O3
512 exchanges, 2,048 clocks). Owner llrm-target-x87, exit 2026-11-20.

## Runs
After `FloatFold` (so a load that folds has folded), before `FloatAssign`.
