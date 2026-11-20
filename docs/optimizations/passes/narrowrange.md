> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/narrowrange.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/narrowrange.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/narrowrange.rs sha256:2de1af8b6a4c9052b54b24d0eccbece6130fcb626324c499e26cb4b719db4afb -->

# narrowrange

An unsigned divide or remainder whose operands ScalarEvolution proves narrow
is done in the narrow type.

## How it works
A loop counter `a` of 0..255 reaches `udiv i32 %a, %b` as an `i32`: the
loop's exit test bounds it, and no instruction in the body says so. Where
the target has a narrow divide or multiply (`Tti::range_narrowing_width`:
m68k `divu.w`, 16 bits) the pass reads each operand's unsigned range from
ScalarEvolution: `udiv`/`urem` with both operands in the width become `zext
(udiv iN (trunc a), (trunc b))`, N the   smallest power of two (8 or more)
that holds them: the instruction selector then sees a narrow   divide, whose
result and remainder are known narrow in turn;

DEVIATE NRW-1 (owner m68k, exit 2026-12-05): the pin narrows a divide by
LazyValueInfo ranges only (`CorrelatedValuePropagation::narrowUDivOrURem`),
which carry no loop bound, so its `div8_os`-like loops call `__udivsi3` and
`__mulsi3` as ours did. The source is gcc's VRP, which refines the range of
an induction variable by its number of iterations (`adjust_range_with_scev`,
tree-vrp.cc) and expands the divide by the operands' `nonzero_bits`
(`expand_divmod`, expmed.cc). Exit: LVI reads loop bounds, or this is
deleted.

## Runs
After `remidiom`, in the strength slot ([`crate::pipeline::strength_slot`](pipeline.md)),
where the target gives a width, at -O2 and -O3: at -O1 and -Os the libcall's
pushes are fewer bytes than the extensions the narrow divide needs (div8_os
68000: +20 B at both, -14% and -38% clocks).

## Example
A divide whose operands ScalarEvolution proves to be 0..255 is done in 8 bits.

```text
%q = udiv i32 %a, %b        ; %a, %b in 0..255
=>
%x = trunc i32 %a to i8
%y = trunc i32 %b to i8
%d = udiv i8 %x, %y
%q = zext i8 %d to i32
```
