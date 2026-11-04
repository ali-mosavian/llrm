> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_casts.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_casts.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_casts.rs sha256:971cea0b94141704f6d5e88d4ea564b7a0e974d7fb0182c91fbce664741fc215 -->

# combine_casts

InstCombine's `visitTrunc`, `visitZExt`, `visitSExt` and the helpers they
call, from
[`InstCombineCasts.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp)
at d1106deb71cc.

`visit_trunc` is
[`visitTrunc`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp#L1045),
`visit_zext` is
[`visitZExt`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp#L1593),
`visit_sext` is
[`visitSExt`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp#L1915):
the same steps in the same order, each helper named for the C++ one with
its own permalink. A step that returns `nullptr` there returns `None` here,
one that returns an instruction returns `Some(Visit)`.

MIRROR: the order of the steps and each condition.

DEVIATE 1: MIR has no vectors and no `vscale`: `foldVecTruncToExtElt`,
`foldVecExtTruncToExtElt`, `shrinkSplatShuffle`, `shrinkInsertElt`,
`foldExtractionOfVectorDeinterleave`, the shuffle step of
`commonCastTransforms` and the `vscale` folds are not translated.

DEVIATE 4: the folds that build `uadd.sat` are not made: the selector has
no `uadd.sat` lowering (`trunc(u/smin(zext a + zext b, MAX))`).

DEVIATE 5: integers wider than 128 bits are left alone (masks are `u128`);
a constant `ConstantExpr` would keep as an expression, or a shift amount
that is not an integer constant, skips the fold.

## Used by
`combine::visit`, once the casts family's slot is flipped.

## How it works
Each `visit_*` returns the first step that changes the cast. A tree of
one-use integer operations under a cast is rebuilt in the cast's type
(`canEvaluate*` asks, `EvaluateInDifferentType` builds), which removes the
cast; a cast of a cast, of a compare or of a shift pair folds to a mask,
a shift or a compare.

## Example
`trunc (add (zext x), 1)` is computed in the narrow type:

```text
%a = zext i8 %x to i32
%b = add i32 %a, 1
%r = trunc i32 %b to i8   ; %r = add i8 %x, 1
```
