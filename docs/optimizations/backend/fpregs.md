> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/fpregs.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/fpregs.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/fpregs.rs sha256:bc2809c10936a5d128c863465937e11bf38bd7fae76163ec379df6d4cc1752e1 -->

# fpregs

The x87 registers FP0..FP6 as the values the stackifier tracks.

## How it works
Under RFP the allocator leaves each x87 operand in
`FP0`..`FP6` (`Loc::Reg`), as `X86FloatingPoint` finds it. `fpstack` follows
values, not registers, so each definition of a register here is a fresh
value and each read the value last defined. A register live across a block
border keeps one value for the whole function (the border value of its
slot), so the values at both ends of an edge agree; the last definition in a
block of a register live out takes that value.

DEVIATE 1: LLVM reads the registers; the stackifier's identity is a value
number (`fpstack` DEVIATE 1). Exit: `fpstack` follows registers.

## Runs
At the start of `fpstack::converted`; a body with no x87 register operand
is returned as it is.

## Example
`fp1 := fld [a]; fp1 := fmul fp1, fp0` with `fp1` live out becomes
`v10 := fld [a]; b1 := fmul v10, b0`, `b1` being `fp1`'s border value.

## Needs
[`crate::backend::allocate::RFP`](allocate.md).
