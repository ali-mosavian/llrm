> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lsr_solve.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lsr_solve.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lsr_solve.rs sha256:c6e32c9ef8c48245ebe95271ea667b576070e9d45caf751dc2413b28da63a91f -->

# lsr_solve

The solver of LLVM's loop strength reduction: one formula per use, by a
depth-first search pruned by the best cost found (`SolveRecurse` and
`Solve`; LoopStrengthReduce.cpp at `d1106deb71cc`, l.5573-5710).

## How it works
Uses are decided in order. A formula of a use must reference every register
already chosen that the use could also reference (up to its register count),
which prunes most of the search; its cost is rated on top of the cost so
far, and the branch is dropped when it is no better than the best solution.
A formula of the first use that is a single register is remembered as
visited: formulae later in the search that name it lose.

## Example
The array walk and its exit test: the store keeps `{%p,+,4}`, the compare
keeps `{(-1 + %n),+,-1}`, one register each, as `opt -passes=loop-reduce`
has them.

## Runs
Nothing runs it yet: stage 5 of the translation of LSR.

## Limits
DEVIATE: `AMK_PostIndexed` (x86 prefers none) is not ported.
