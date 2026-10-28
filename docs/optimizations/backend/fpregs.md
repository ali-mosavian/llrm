> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/fpregs.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/fpregs.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/fpregs.rs sha256:63a6a2b6fddef68c7b43f1d4bd9a30cdb177570c860a8b96404e8b12ae5b9659 -->

# fpregs

The x87 registers FP0..FP6 as the values the stackifier tracks.

## How it works
Under RFP the allocator leaves each x87 operand in `FP0`..`FP6`
(`Loc::Reg`), as `X86FloatingPoint` finds it. The stackifier's identity is a
value number, so each register is one value for the whole function: every
write and read of `FP1` names the same number, and the values at both ends
of an edge agree.

DEVIATE 1: LLVM reads the registers; the stackifier's identity is a value
number (`fpstack` DEVIATE 1), here the register's own.

## Runs
At the start of `fpstack::converted`; a body with no x87 register operand
is returned as it is.

## Example
`fp1 := fld [a]; fp1 := fmul fp1, fp0` becomes
`b1 := fld [a]; b1 := fmul b1, b0`, `b1` being `fp1`'s value.

## Needs
[`llrm_lir::registers::Regs::rfp_registers`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-lir/src/registers.rs).
