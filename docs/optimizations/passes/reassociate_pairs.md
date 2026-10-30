> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/reassociate_pairs.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/reassociate_pairs.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/reassociate_pairs.rs sha256:305fbd0d372d20e446b2ceb04482d827194985d83eab8dd14c1d469e0f6a4089 -->

# reassociate_pairs

Reassociate's pair map: how often two operands are combined by the same
operation, so that the most popular pair is formed first and CSE finds it.

Translation of `BuildPairMap` and the choice of the best pair in
`ReassociateExpression`
([Reassociate.cpp:2669](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L2669),
[:2543](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L2543))
at d1106deb71cc.

MIRROR: only the root of each tree is counted; a tree of more than
`GLOBAL_REASSOCIATE_LIMIT` leaves is skipped, and so is the choice for an
expression of more; with `reassociate-use-cse-local` (on) the reordering
stays among the leaves of the first block seen.

## How it works
A hash map from (operation, operand, operand) to a count: one pass over the
function, at most 45 pairs per tree (the limit squared over two), so O(n).
The choice scans the pairs of one tree, also bounded by the limit.

DEVIATE: a pair is ordered by value id, not by address; ids are never
reused, so a key cannot go stale (`PairMapValue::isValid`).

## Used by
`crate::reassociate`, `crate::reassociate_inst`.

## Example
In `a*b*c` and `a*b*d`, the pair `a*b` counts 2: `a*b` is formed first in
both and the second is a common subexpression.
