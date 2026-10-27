> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lsr_narrow.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lsr_narrow.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lsr_narrow.rs sha256:468d7dc2f2b32f35dc0f38282c7cc9ad56294bec0f92d5b53de573e3d6436432 -->

# lsr_narrow

How LLVM's loop strength reduction thins the formulae when there are too
many for the solver (`NarrowSearchSpaceUsingHeuristics` and the heuristics
it runs; LoopStrengthReduce.cpp at `d1106deb71cc`, l.4968-5560).

## How it works
The search space is the product of the number of formulae of each use. Past
the complexity limit (65535, LLVM's), the heuristics run in order, each
only if the space is still too big: drop formulae that use a superset of
the registers of another, merge uses a constant offset apart (unrolled
code), filter again, keep the best formula per scaled register and scale,
merge uses outside the loop into those inside, and last pick the register
most uses share and drop what does not use it, until the space fits.

## Example
Four uses of twenty formulae each (160000 combinations) with a register all
share: the formulae without it go, one is left per use.

## Runs
Nothing runs it yet: stage 4 of the translation of LSR.

## Limits
DEVIATE: `NarrowSearchSpaceByFilterPostInc` (post-indexed addressing, x86
prefers none) and `NarrowSearchSpaceByDeletingCostlyFormulas`
(`-lsr-exp-narrow`, off by default) are not ported.
