> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/transform.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/transform.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/transform.rs sha256:72bd44a66700348166683f3f6cf6f480af646b8cb632b4b121f5dcfcf63ef1ee -->

# transform

The helpers [`crate::gvn`](gvn.md), [`crate::dead`](dead.md), [`crate::decide`](decide.md),
share: equal calls, liveness of
values, and cleanup of phis after an edge goes.

## How it works
`equal_calls` makes two calls with one callee and arguments one when the
caller's `silent` says they touch no memory: the later takes the earlier's
result where the earlier has certainly run.

`live` marks, to a fixed point, every value that something staying reads;
`dead` removes the rest. `_comparison` finds the `icmp` in a block that
defines a branch's condition. `_trivial_phis` resolves joins after an edge
disappears: each phi keeps only the inputs of its remaining predecessors, a
phi with one other value is that value, and a phi with the arms of an
earlier phi in its block is that phi (LLVM's `EliminateDuplicatePHINodes`).

## Example
No honest example: these are helpers shared by passes, and none changes a
function on its own. Read [`crate::gvn`](gvn.md) and its tests for them in use.

## Provides
`equal_calls` ([`crate::gvn`](gvn.md)); `live` and `live_except` ([`crate::dead`](dead.md),
[`crate::floatloop`](floatloop.md)); `_comparison` ([`crate::decide`](decide.md));
`_trivial_phis` ([`crate::decide`](decide.md)).

## Used by
[`crate::gvn`](gvn.md) calls `equal_calls` on each function it numbers;
[`crate::dead`](dead.md) and [`crate::floatloop`](floatloop.md) call `live` to decide what to
delete; [`crate::decide`](decide.md)
calls `_trivial_phis` after it drops an edge. Not a
pass: it runs only inside those.

## Needs
`cfg::Dominance` (`llrm_analysis`).
