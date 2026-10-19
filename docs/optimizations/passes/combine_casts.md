> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_casts.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_casts.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_casts.rs sha256:1eba8acaffa44eb9d6e431f442a9ed479d6b9cfde22e51fb23c632d003db4481 -->

# combine_casts

InstCombine's `visitTrunc` and the helpers it calls, from
[`InstCombineCasts.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp)
at d1106deb71cc.

`visit_trunc` is
[`visitTrunc`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp#L1045):
the same steps in the same order, each helper named for the C++ one with
its own permalink. A step that returns `nullptr` there returns `None` here,
one that returns an instruction returns `Some(Visit)`. `visitZExt` and
`visitSExt` follow in their own change.

MIRROR: the order of the steps and each condition.

DEVIATE 1: MIR has no vectors and no `vscale`: `foldVecTruncToExtElt`,
`foldVecExtTruncToExtElt`, `shrinkSplatShuffle`, `shrinkInsertElt`,
`foldExtractionOfVectorDeinterleave`, the shuffle step of
`commonCastTransforms` and the `vscale` folds are not translated.

DEVIATE 2 (temporary): `FoldOpIntoSelect`, the select step of
`commonCastTransforms`, is left out until the select family lands it.

DEVIATE 3 (temporary): `SimplifyDemandedInstructionBits` is not
translated.

DEVIATE 4 (temporary): the folds that build an intrinsic call (`uadd.sat`,
`usub.sat`, `smin`, `smax`, `ctlz`, `scmp`, `ucmp`, `fshl`, `fshr`) are
left out until the Builder can declare an intrinsic (`combine_build`
DEVIATE 2). So is the `llvm.abs` and min/max call case of
`canEvaluateTruncated` and `EvaluateInDifferentType`: a call is not
evaluable in the narrow type here.

DEVIATE 5: integers wider than 128 bits are left alone (masks are `u128`);
a constant `ConstantExpr` would keep as an expression, or a shift amount
that is not an integer constant, skips the fold.

## Used by
`combine::visit`, once the casts family's slot is flipped.

## How it works
`visit_trunc` returns the first step that changes the cast. A tree of
one-use integer operations under the trunc is rebuilt in the narrow type
(`canEvaluateTruncated` asks, `EvaluateInDifferentType` builds), which
removes the trunc; a trunc of a binary operation, a shift or an `i1` test
folds to a narrow operation or a compare.

## Example
`trunc (add (zext x), 1)` is computed in the narrow type:

```text
%a = zext i8 %x to i32
%b = add i32 %a, 1
%r = trunc i32 %b to i8   ; %r = add i8 %x, 1
```
