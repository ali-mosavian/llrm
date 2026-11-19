> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/floativ.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/floativ.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/floativ.rs sha256:078de7fb13a14dcf9837b3c1275a1ad172b827e35fe8970b116b832b54f4c8fe -->

# floativ

A counter that steps a `float` or `double` by a constant becomes an `i32`
counter, so ScalarEvolution can count the loop. LLVM's IndVarSimplify
`rewriteNonIntegerIVs` (`handleFloatingPointIV`).

## How it works
For each header phi `%x = phi [C0, pre], [%n, latch]` with
`%n = fadd %x, C1`, whose only other use is an `fcmp` against a constant
that alone decides the exit branch of the loop: the start, step and bound
are whole numbers a signed 32-bit integer holds (and the float's mantissa
holds exactly), and the integer counter reaches the bound without wrapping.
Then an `i32` phi, add and `icmp` take the place of the three, and a read
of `%x` elsewhere in the loop becomes `sitofp` of the integer phi.

## Example
`for (double x = 0; x < 100; x += 1) use(x)` becomes `for (int i = 0;
i < 100; i += 1) use((double)i)`.

## Used by
[`crate::indvars`](indvars.md), first, as the pin runs it.

## See also
LLVM `IndVarSimplify.cpp`. Tests: `floativ_tests.rs`.
