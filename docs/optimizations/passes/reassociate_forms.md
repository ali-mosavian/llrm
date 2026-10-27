> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/reassociate_forms.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/reassociate_forms.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/reassociate_forms.rs sha256:40a294b06d3b609eafd10ec0992b44efe125031895ccce81ba96f0695fb0df3b -->

# reassociate_forms

Reassociate's rewrites into forms that join a tree: a shift by a constant
as a multiply, an `or` of disjoint bits as an add, a product of a sum as a
sum of products.

Translation of `ConvertShiftToMul`, `shouldConvertOrWithNoCommonBitsToAdd`,
`isLoadCombineCandidate`, `convertOrWithNoCommonBitsToAdd`,
`ShouldBreakUpDistribution` and `BreakUpDistribute`
([Reassociate.cpp:927-1161](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L927))
at d1106deb71cc.

MIRROR: each applies only where it joins a tree of the operation it turns
into; an `or` that looks like a load widening is left alone; `(X+Y)*C` is
distributed only where a sibling `X*C'` or `Y*C'` will factor again.

## How it works
Each is a local rewrite of one instruction and its immediate users and
operands; the walk in `isLoadCombineCandidate` visits each instruction once.

## Used by
`crate::reassociate_inst` (`OptimizeInst`).

## Example
`(x << 3) * y` becomes `(x * 8) * y`; `x | y` with no common bits, added
to `z`, becomes `x + y + z`.
