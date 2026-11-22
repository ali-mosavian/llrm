> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_fneg.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_fneg.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_fneg.rs sha256:ab6d8531537a6642a5bd2e147399ab4c57cde73c37faa8edc844bdb514f8eeb7 -->

# combine_fneg

`visitFNeg`: an `fneg` of a one-use value is pushed into the value.

Translation of
[`InstCombinerImpl::visitFNeg`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp#L3191),
[`foldFNegIntoConstant`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp#L3096)
and
[`hoistFNegAboveFMulFDiv`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp#L3157).

MIRROR: the C++ order: `foldFNegIntoConstant`, `-(X - Y)` under `nsz`, then
(the operand having one use) the hoist above `fmul`/`fdiv`, then the select
arms. The flag algebra of `foldFNegIntoConstant` (`unionValue`,
`intersectRewrite`, `ninf` of both) is the pin's.

DEVIATE 1: `simplifyFNegInst` is the generic simplifier `visit` runs first.

DEVIATE 2: the `ldexp` arm of the hoist, `fneg (copysign x, y)`, and the
vector arms (shuffle, reverse) are not translated: MIR has no vectors, and
`copysign` and `ldexp` calls need `CreateCall`/`CreateCopySign`. The
`copyMetadata` of the `fdiv` arms is not done: MIR instructions carry no
metadata here. A constant other than a `float` or `double` is not negated
(`negated_constant`).

## Used by
`combine::visitors` for `fneg`.

## Example
```text
%m = fmul float %x, 2.0 ; %n = fneg float %m   ==>   %n = fmul float %x, -2.0
%m = fmul float %x, %y  ; %n = fneg float %m   ==>   %0 = fneg float %y ; %n = fmul float %x, %0
```
