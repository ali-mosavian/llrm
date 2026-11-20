> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/vectorutils.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/vectorutils.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/vectorutils.rs sha256:bb269fd6cdc4bfa254179f633a289faed89a7a987740768a962fcfa455bc0f61 -->

# vectorutils

`VectorUtils`: which intrinsic calls a vectorizer may widen.

A translation of LLVM `d1106deb71cc` `VectorUtils.cpp`:
`getVectorIntrinsicIDForCall`
([L234](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/VectorUtils.cpp#L234)),
`isTriviallyVectorizable` (L46) and `isVectorIntrinsicWithScalarOpAtArg`
(L142). Users: `LoopAccessInfo` (a widenable call reads no memory the
analysis must order) and `LoopVectorizationLegality` (the call arm of
`canVectorizeInstrs`).

## Example
`%m = call i16 @llvm.smax.i16(i16 %v, i16 7)` is trivially vectorizable;
`llvm.ctlz.i32(%v, i1 false)` is too, its second argument staying scalar.

## DEVIATE rows
- VU1 `llvm.powi` and `llvm.ldexp` (a scalar operand that is not an
  immediate): MIR has no intrinsic for them, so a call of either is not an
  intrinsic here. Exit: when MIR names them.
- VU2 the `VFDatabase` and `isTLIScalarize` ways of accepting a call: no
  vector function mappings exist.

## Used by
Named by `loopaccessinfo`, `loopvectorizelegality`, `slptree`.
