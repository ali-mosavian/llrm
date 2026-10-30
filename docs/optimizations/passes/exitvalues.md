> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/exitvalues.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/exitvalues.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/exitvalues.rs sha256:9fa8b0113259e1a2a451191512eba8e0a77fa7a1b3eb4fe453d6a75b6aa8f7d9 -->

# exitvalues

A value a loop computes and the code after it reads becomes an expression
of the loop's trip count, written where the loop is left. LLVM's
`rewriteLoopExitValues` (`LoopUtils.cpp`), which IndVarSimplify calls with
its default policy `OnlyCheapRepl`.

## How it works
For each loop, innermost first, and each phi of its exit blocks (loop-closed
SSA puts every value read after the loop there), the incoming value defined
in the loop is asked its value at the exit (`getSCEVAtExit`). It is
rewritten when that is invariant in the loop and safe to expand, unless the
loop itself reads the value in a way that cannot go away (an instruction
with side effects reached from it: `hasHardUserWithinLoop`). The expansion
is written before the defining instruction (the first non-phi of its block
for a phi) and must be cheap (`isHighCostExpansion`, budget 4), unless the
loop can then be deleted (`canLoopBeDeleted`: one exiting block, one exit,
nothing in the loop with side effects, the exit's other phis invariant).
The phi reads the expansion; a phi left with one incoming value is replaced
by it.

## Example
`s = 0; for (i = 0; i < n; i++) s += 2; return s` returns `2 * n`: the exit
phi reads `shl n, 1` written in the exit block, and the loop's `s` is dead.

## Used by
[`crate::indvars`](indvars.md), first of its steps. It replaced indvars' own
`exit_values`, which asked only the header for its exit.

## See also
`rewriteLoopExitValues`, `canLoopBeDeleted`, `hasHardUserWithinLoop`,
`docs/architecture/scev-port.md` (D11, D12, D13).
