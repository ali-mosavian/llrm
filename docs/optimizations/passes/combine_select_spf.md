> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_select_spf.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_select_spf.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_select_spf.rs sha256:181b1ae626389bd133918efb0bdfb73968d7530feaba2ece3ca74adbb4b02e65 -->

# combine_select_spf

`foldSelectInstWithICmp`'s first step: a select that is a min, a max or an
absolute value of a compare becomes the intrinsic.

Translation of
[`canonicalizeSPF`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L1623)
at d1106deb71cc, reached from
[`foldSelectInstWithICmp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L2419).

MIRROR: `matchDecomposedSelectPattern` of the compare and the two arms;
`smin`, `smax`, `umin` and `umax` for the min/max flavors; `abs` (poison at
the minimum when the other arm is the `nsw` negation of the first) for the
absolute value, with `0 - abs` for its negation; an absolute value is
skipped when neither the compare nor its negation has one use.

DEVIATE 1: `foldSelectInstWithICmp`'s other steps
(`foldSelectInstWithICmpConst`, `canonicalizeClampLike`, ...) are not
translated here; the min/max of `FoldSelectWithAndOrCond` is the next strip.

## Used by
`visitSelectInst`.

## Example
```text
%c = icmp slt i16 %x, 0                 %r = call i16 @llvm.smax.i16(i16 %x, i16 0)
%r = select i1 %c, i16 0, i16 %x   ==>
```
