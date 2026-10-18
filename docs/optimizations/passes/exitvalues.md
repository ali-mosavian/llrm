> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/exitvalues.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/exitvalues.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/exitvalues.rs sha256:ddb510cf743705b1d57d725fc1bca33fb8204a927c44c11cd2a697dc542382ef -->

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
[`crate::indvars`](indvars.md), first of its steps, in place of
[`crate::indvars::exit_values`](indvars.md) when `LLRM_NO_SCEV_EXITVALUES` is not set.

## See also
`rewriteLoopExitValues`, `canLoopBeDeleted`, `hasHardUserWithinLoop`,
`docs/architecture/scev-port.md` (D11, D12, D13).
