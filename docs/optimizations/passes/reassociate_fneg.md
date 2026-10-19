> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/reassociate_fneg.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/reassociate_fneg.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/reassociate_fneg.rs sha256:74e386716f11d76850a9fece0cec43b296f066181dfe2ec0ba42c47243a84ee0 -->

# reassociate_fneg

Reassociate's negative floating-point constants: a negated constant out of
a product, so that `fadd` becomes `fsub` and constants fold and CSE.

Translation of `getNegatibleInsts`, `canonicalizeNegFPConstantsForOp` and
`canonicalizeNegFPConstants`
([Reassociate.cpp:2165-2288](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L2165))
at d1106deb71cc.

MIRROR: the one-use `fmul`/`fdiv` expression under an `fadd`/`fsub` is
searched for negative constants; each is made positive, and an odd number
turns the `fadd` into an `fsub` (or back) so that the sign is not lost.

DEVIATE: `getNegatibleInsts` keeps its own stack, not recursion: a chain of
one-use products is as deep as the function is long.

## Used by
`crate::reassociate_inst` (`OptimizeInst`).

## Example
`x + (y * -2.0)` is `x - (y * 2.0)`.
