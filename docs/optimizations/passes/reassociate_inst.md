> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/reassociate_inst.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/reassociate_inst.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/reassociate_inst.rs sha256:d3f6e41d40dacb3ff5097e5c1b2697069853c38ae34c5dee34100206b136fb14 -->

# reassociate_inst

Reassociate of one instruction: put it in a form that joins a tree, and if
it is the root of a tree, reassociate the tree.

Translation of `OptimizeInst`, `canonicalizeOperands` and
`ReassociateExpression`
([Reassociate.cpp:2292](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L2292),
[:290](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L290),
[:2442](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L2442))
at d1106deb71cc.

MIRROR: the order of the steps (shift to multiply, commute by rank, fold
negative floating constants, `or` to add, distribute, break up a subtract,
lower a negate); an interior node of a tree waits for its root so that a
tree is analyzed once.

## How it works
The steps are a chain over one instruction; each step may replace the
instruction by the one it made. A root linearizes its tree, sorts the leaves
by rank (stable, highest first), optimizes the list, chooses the pair for
CSE and writes the list back.

## Used by
`crate::reassociate` (`visit_block`).

## Example
`(a + 4) + b` where `b` has the lower rank becomes `(b + a) + 4`.
