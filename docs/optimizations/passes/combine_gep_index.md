> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_gep_index.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_gep_index.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_gep_index.rs sha256:5b006a91c514ff0d577eadc08c6e59b65b540f7381b794ff6ad703a10ba3dca8 -->

# combine_gep_index

The rewrites of `visitGetElementPtrInst` that read its indices and flags
(:3513-3705): a scaled single index, `inbounds` and `nuw` the object or the
indices prove, a sum split in two GEPs, a select of constants, and `srem`
as `urem`.

Translation of
[`InstCombinerImpl::visitGetElementPtrInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L3513)
and `foldSelectGEP` (:2759) at d1106deb71cc. Each is a `Fold` of
`combine_gep::FOLDS`.

DEVIATE 1: `ptrtoaddr` is not a MIR instruction: only `ptrtoint` users and
compares take the pointer a difference of pointers rebuilds.

## Example
```text
%i = add i64 %x, %y
%p = getelementptr i32, ptr %b, i64 %i         %q = getelementptr [4 x i8], ptr %b, i64 %x
                                         ==>   %p = getelementptr [4 x i8], ptr %q, i64 %y
```

## Used by
Named by `combine_gep`.
