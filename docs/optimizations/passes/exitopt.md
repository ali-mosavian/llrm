> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/exitopt.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/exitopt.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/exitopt.rs sha256:2c3a1f522d11177ad9babe728ecbe30d98f96fd1e158dc453ada34fbf1909a52 -->

# exitopt

A loop's exits are folded by what ScalarEvolution knows of their counts.
LLVM's `optimizeLoopExits` (`IndVarSimplify.cpp`) with
`optimizeLoopExitWithUnknownExitCount`.

## How it works
For each loop in LoopSimplify form, innermost first, the exits that are in
the loop itself, end in a conditional branch on a non-constant, and
dominate the latch are taken in dominance order. A constant exit that is
always taken has the header phis replaced by their preheader values.

- Exact count unknown: each `icmp` leaf of the exit's `and`/`or` tree is
  replaced by the compare of the counter's start when ScalarEvolution proves
  the compare is invariant over the loop's first iterations, or by the
  constant it decides (`getLoopInvariantExitCondDuringFirstIterations`).
- Count zero: the exit is taken on the first trip.
- Count above the loop's maximum, or equal to a dominating exit's: the exit
  is never taken.

A decided exit gets the constant condition that decides it; `dead` and
`simplifycfg` remove the rest.

## Example
The second exit `i < len` with `i < n` guarded by `n <= len`:
`br i1 %lt, label %loop, label %exit` becomes
`br i1 true, label %loop, label %exit`.

## Used by
Not wired in: [`crate::indvars`](indvars.md) will run `optimized` before
[`crate::lftr`](lftr.md), as `IndVarSimplify::run` does.

## See also
`docs/architecture/scev-port.md`.
