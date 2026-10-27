> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/reassociate_optimize.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/reassociate_optimize.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/reassociate_optimize.rs sha256:d85e4613ba0f60f5d8f4a7db312512fa7b43bb7ac12a49d314fc5822b8196037 -->

# reassociate_optimize

Reassociate's optimizers of a linearized expression: the identities of one
operation applied to its sorted leaves.

Translation of `OptimizeExpression`, `OptimizeAndOrXor`, `OptimizeXor`,
`OptimizeAdd`, `OptimizeMul`, `RemoveFactorFromExpression`,
`buildMinimalMultiplyDAG` and their helpers
([Reassociate.cpp:1166-2105](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L1166))
at d1106deb71cc.

MIRROR: constants fold to one; `X & X`, `X ^ X`, `X & ~X`, `X + -X` go;
repeated addends become a multiply; a factor common to several products is
pulled out; a product of four or more repeated factors becomes a minimal
multiply DAG; `xor` operands over one value are merged by the four Xor
rules.

DEVIATE 1: the `xor` rules work on integers up to 128 bits; a wider `xor`
is left as `OptimizeAndOrXor` leaves it.

DEVIATE 2: `FindSingleUseMultiplyFactors` and `EmitAddTreeOfValues` are
loops, not recursion: a product or sum of a function's length would
otherwise overflow the stack. The order of the factors and of the adds made
is upstream's.

DEVIATE 3: a floating constant is the nearest `f32` or `f64`; see
`reassociate_fp`.

## How it works
Each optimizer works on `ops`, sorted by rank, high to low, so equal values
and a value and its negation are adjacent. `FindInOperandList` scans only
the run of equal rank. The caller recurses while the leaf count falls.

## Used by
`crate::reassociate` (`ReassociateExpression`).

## Example
`x + x + y` has leaves `x, x, y` and becomes `x*2 + y`.
