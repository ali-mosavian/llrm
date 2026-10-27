> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/loopunrollanalyzer.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/loopunrollanalyzer.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/loopunrollanalyzer.rs sha256:e4a99a9c4fdbb0a972a60ba6992fe7eb91bd6deb7b131e4ad2b752fdde4725df -->

# loopunrollanalyzer

`UnrolledInstAnalyzer`: what simplifies in one iteration of a loop if it
were unrolled completely, to cost the unrolled form without making it.

## How it works
The caller walks the loop's instructions iteration by iteration, with a map
of the simplified values so far (header phis of the iteration included).
`visit` answers whether an instruction is free after unrolling, and records
what it simplifies to: an operation on operands the map already holds is
simplified (`simplify_binop_operands`, compares, casts); a load from a
constant global at an offset that ScalarEvolution makes constant at this
iteration is folded to the initializer's element; any other instruction
asks ScalarEvolution for its expression, which is a constant, or an add
recurrence of the loop evaluated at the iteration, or is loop-invariant
(free after the first iteration). A pointer that is the base of a global
plus an offset known at this iteration is remembered with that offset.

## Deviations
- U1 `fcmp` is not simplified: no `simplifyFCmpInst` exists in
  `llrm_mir::simplify`; it is asked of ScalarEvolution alone.
- U2 No `FPMathOperator` fast-math flags beyond those on the instruction.

## Example
At iteration 2 of a loop with `%i = phi i16 [ 0, %pre ], [ %n, %latch ]`,
`%i` is `2`, so `%n = add i16 %i, 1` simplifies to `3`, `icmp slt i16 %n, 4`
to `true`, and neither is charged to the unrolled loop.

## See also
[`UnrolledInstAnalyzer`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopUnrollAnalyzer.cpp).

## Used by
Named by `loopunrollpass`.
