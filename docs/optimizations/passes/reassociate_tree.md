> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/reassociate_tree.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/reassociate_tree.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/reassociate_tree.rs sha256:4477250c3609a6813922a0fe144e698036c3e4b25df39e5b9119f7c17af1a630 -->

# reassociate_tree

Reassociate's expression trees: collect the leaves of a tree of one
associative operation with their weights, and write a new leaf list back
into the tree's own nodes.

Translation of `LinearizeExprTree` and `RewriteExprTree`
([Reassociate.cpp:436](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L436),
[:626](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L626)),
`OverflowTracking` (Local.h:581) and `isFMulAddCandidate` (:189) at
d1106deb71cc.

MIRROR: a node of the tree has one use; a value met again with all its
uses inside the tree is morphed (a `neg` in a multiply tree becomes a
multiply by -1), not a leaf. Nodes the optimizers leave unused are reused
for the rewritten tree before a new one is made, and the flags that no
longer hold are cleared from the first changed node up to the root.

## How it works
`linearize_expr_tree` is one pass over the tree: O(size).
`rewrite_expr_tree` walks the new leaf list once, O(leaves).

## Used by
`crate::reassociate` (`ReassociateExpression`) and
`crate::reassociate_optimize` (`RemoveFactorFromExpression`).

## Example
`((a + b) + c) + a` linearizes to the leaves `a` (weight 2), `b`, `c`.
