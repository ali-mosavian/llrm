> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_fpcast.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_fpcast.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_fpcast.rs sha256:c8ba0266b2d9b9b55e2b42d671665a099447eeacc04543daab4d7889ca54ac67 -->

# combine_fpcast

InstCombine's `visitUIToFP`, `visitSIToFP` and `visitFPExt`, from
[`InstCombineCasts.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp)
at d1106deb71cc.

`visit_uitofp` is
[`visitUIToFP`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp#L2626),
`visit_sitofp` is `visitSIToFP` (:2656) and `visit_fpext` is
`visitFPExt` (:2450). Each opens with, or ends in, `commonCastTransforms`,
which is what folds `sitofp (sext X)` to `sitofp X`.

MIRROR: the order of the steps and each condition.

DEVIATE 1: MIR has no vectors.

## Example
```text
%w = sext i32 %x to i64                %d = sitofp i32 %x to double
%d = sitofp i64 %w to double
```

## Used by
`combine` (its `visit`, the `UIToFP`, `SIToFP` and `FPExt` opcodes).
