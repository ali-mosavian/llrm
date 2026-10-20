> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/candidate.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/candidate.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/candidate.rs sha256:73ef39e86bdcf6847d94bad9284ce3bcb122eb2d601a7453aa23804852c99e7b -->

# candidate

Trying a rewrite on a copy of the body: the copy is cleaned by the
pipeline's own passes and replaces the body only if it is better.

## How it works
`adopted(unit, outer, costs, candidate, reoptimize, pricing, gain)` runs
`reoptimize` over `candidate`, then compares it with the body in `unit`. By
`Gain::FewerLoops` it is kept only when it has fewer loops, no more
operations and no larger price; by `Gain::CheaperBy(percent)` when its price
is smaller by that percent. A body with an operation the target does not
price is never better.
The price is [`crate::profit::weighted`](profit.md) (`Pricing::Work`) or
[`crate::profit::motion_price`](profit.md), which adds what the registers spill
(`Pricing::WithSpills`), each block weighted by the trips `induction`
proves, as [`crate::profit::_frequencies`](profit.md) has them.

```text
  body ---- rewrite ----> candidate ---- reoptimize ----> candidate'
    |                                                        |
    +----------- fewer loops, no more ops, price <= -------- +  keep candidate'
```

## Example
None: a library. [`crate::loopversion`](loopversion.md) judges a specialised loop by work
alone.

## Provides
`adopted`, `priced`, `priced_at`, `frequencies`, `Pricing`, `Gain`,
`Baseline`.

## Needs
[`crate::profit`](profit.md) and the target's operation costs.

## See also
LLVM's `SimpleLoopUnswitch` cost check (`computeDomSubtreeCost`) and
gcc's `loop_unswitch` size limits are bounds before the copy; this is the
judgement after it.
