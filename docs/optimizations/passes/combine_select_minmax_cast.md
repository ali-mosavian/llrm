> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_select_minmax_cast.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_select_minmax_cast.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_select_minmax_cast.rs sha256:0b51a90d7e7e16adff91a051326166d5fc3aac460a0bd63984db57bc594a198e -->

# combine_select_minmax_cast

`visitSelectInst`'s min/max canonicalisation: a select that is a min or a
max of values behind a cast is rebuilt on the uncast values and the cast is
moved outside.

Translation of
[`visitSelectInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L4991)
at d1106deb71cc (the `isMinOrMax` block).

MIRROR: `matchSelectPattern` with a `CastOp`; the compare is rebuilt with
`getMinMaxPred` (a floating-point one carries the old compare's fast-math
flags); the select takes the old one's `prof`; the result is cast with the
pattern's `CastOp`.

DEVIATE 1: `foldSPFofSPF` (the step before this block) is not translated.

## Used by
`visitSelectInst`.

## Example
```text
%c = fcmp oge float %p, 6.5535e4                %q = select i1 %c, float 6.5535e4, float %p
%t = fptosi float %p to i64              ==>    %r = fptosi float %q to i64
%r = select i1 %c, i64 65535, i64 %t
```
