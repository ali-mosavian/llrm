> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/floatalloc.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/floatalloc.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/floatalloc.rs sha256:636adb18c1d8b0a08114bab5e930e8a191c4c86722251edd011ce091a5b7a7cd -->

# floatalloc

The x87 phase after register allocation: copy propagation, the stackifier
and the `fisttp` mode switching, with the helpers they share.

## How it works
`FloatAlloc::transform` runs `fpcopyprop::propagated`, then
[`crate::backend::fpstack::converted`](fpstack.md) (the translation of LLVM's
`X86FloatingPoint`, which turns the flat registers `RegAlloc` chose into
stack form), then `_truncating`.

`_truncating` makes `fisttp` as a 387 has it: `fistp` between `fldcw` of a
control word with rounding set toward zero and a `fldcw` of the saved one.
The two words are saved once at entry.

Where the switches go is gcc's mode switching, in one analysis and one
placement over the graph (`_needs`, `_modes`;
[`optimize_mode_switching`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/mode-switching.cc#L495),
the i386 entity
[`I387_TRUNC`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/config/i386/i386.cc#L14607)).
Each instruction needs a mode: truncating for a conversion, the caller's for
whatever rounds, a call or a return, none for exact work. A block assumes on
entry the mode its first need names, or its successors' (the heavier by loop
depth), and a switch goes on each edge whose ends disagree, so a loop of
conversions switches on its entry edge and its exits, not twice a trip
(`fldcw` is 4 clocks on a 486, 7 on a P5). Unlike gcc's lazy code motion
this does not solve for the least number of insertions: it follows demand
and takes a critical edge's source mode.

## Example
A `fisttp` in a loop body gets one `fldcw` pair on the loop's entry edge
and exit, not one per trip.

## Runs
After `RegAlloc` in the machine phases, once per function, on bodies with
floating values only (the truncation fix-up runs where there is a `fisttp`).
Not gated by an -O level.

## Needs
[`crate::backend::fpstack`](fpstack.md), `fpcopyprop`, [`crate::backend::floatregions`](floatregions.md)
for the instruction boundaries.

## See also
`docs/optimizations/floating-pop-reuse.md`; LLVM `X86FloatingPoint.cpp`.
