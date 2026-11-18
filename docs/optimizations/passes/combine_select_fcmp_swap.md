> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_select_fcmp_swap.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_select_fcmp_swap.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_select_fcmp_swap.rs sha256:831721451f298291015eba9bb8daca684e62d11c7b9bbbf60f87862114ed5115 -->

# combine_select_fcmp_swap

`visitSelectInst`: a select of the two values an unordered `fcmp` compares
takes the ordered compare and swapped arms.

Translation of the `FCmpInst` block of
[`visitSelectInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L4693)
at d1106deb71cc.

MIRROR: the new compare is the inverse predicate with the old compare's
fast-math flags (named `.inv` in the pin); `nnan` and `ninf` of the compare
reach the select; the old compare must have one use.

DEVIATE 1: only this canonicalisation of the block is translated (the
scale-if-equals-zero, `minnum`/`maxnum` and `canonicalize` folds after it
are not).

## Used by
`visitSelectInst`.

## Example
```text
%c = fcmp ult float %p, %q                      %c.inv = fcmp oge float %p, %q
%r = select i1 %c, float %p, float %q    ==>    %r = select i1 %c.inv, float %q, float %p
```
