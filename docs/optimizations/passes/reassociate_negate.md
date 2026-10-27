> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/reassociate_negate.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/reassociate_negate.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/reassociate_negate.rs sha256:34de230f9d4afd4f7e8f8aee0b5ec706d29d58a42c78ca62e36d88ad01bf2d9f -->

# reassociate_negate

Reassociate's subtractions and negations: forms that let a subtract or a
negate join an add or multiply tree.

Translation of `NegateValue`, `ShouldBreakUpSubtract`, `BreakUpSubtract`
and `LowerNegateToMultiply`
([Reassociate.cpp:343](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L343),
[:820](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L820),
[:1081](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L1081),
[:1111](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L1111))
and the negative floating-point constant canonicalization
(:2165-:2288) at d1106deb71cc.

MIRROR: a negate is pushed through a one-use add into the leaves, an
existing negate of a leaf is reused (moved to just after the leaf), a new
one is made otherwise; a subtract becomes an add of a negate when that
joins an add or subtract tree.

DEVIATE 1: `NegateValue` is an explicit stack, not recursion: a chain of
one-use adds is as deep as the function is long. The order in which
negates are placed and operands set is upstream's.

DEVIATE 2: a leaf that is an `invoke` result keeps no reusable negate (the
insertion point after its definition is in another block).

DEVIATE 4 (of `reassociate.rs`): MIR has no constant `sub`, so
`ConstantExpr::getNeg` of a constant expression (`ptrtoint (ptr @g to i32)`)
is a `sub 0, C` instruction before the instruction negated, not a constant
operand. It is used by the add that replaced the subtract (bench row
x_strlen: the pin prints the constant `sub (i32 0, ptrtoint ...)` there).
Exit: a constant `sub` in MIR.

Complexity (not a DEVIATE): `NegateValue` recurses into both operands of
every one-use add under the value it negates
([:840-:862](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L840)),
and `ShouldBreakUpSubtract` breaks up a subtract whose operand is such an
add
([:1093-:1099](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L1093)),
so in a chain of n right-nested subtracts (`a0 - (a1 - (a2 - ...))`) each
break-up negates the tree the last one made: O(n^2), upstream's. A
left-nested chain takes O(n) (`reassociate_tests.rs` times it).

## Used by
`crate::reassociate_tree` (`lower_negate_to_multiply`) and
`crate::reassociate` (`OptimizeInst`).

## Example
`a - (b + c)` becomes `a + (-b + -c)`; `-(x * y)` becomes `x * y * -1`.
