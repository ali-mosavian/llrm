> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_gep_merge.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_gep_merge.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_gep_merge.rs sha256:8d9058cc5e073c8ec820b1446511361f7f384a53d556b917a9094512e3d80e60 -->

# combine_gep_merge

`visitGEPOfGEP`: a `getelementptr` of a `getelementptr` as one.

Translation of
[`InstCombinerImpl::visitGEPOfGEP`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L2885)
with `shouldMergeGEPs` (:2345), `canonicalizeGEPOfConstGEPI8` (:2786) and
`combineConstantOffsets` (:2831) at d1106deb71cc.

MIRROR: the order of the four folds; the flag algebra of
`GEPNoWrapFlags::intersectForOffsetAdd` and `intersectForReassociate`
(`GepFlags`); merged only where the sum of the two indices simplifies and
at most one index of the result is non-zero.

DEVIATE 1: the source `GEPOperator` is a `getelementptr` instruction: MIR
has constant GEP expressions, and they are folded to a base and an offset
by `ConstantFolding` before they reach here.

## Example
```text
%q = getelementptr i32, ptr %p, i64 %i              %r = getelementptr i32, ptr %p, i64 %s
%r = getelementptr i32, ptr %q, i64 1         ==>   (%s = add i64 %i, 1, when that simplifies)
```

## Used by
[`combine_gep`](combine_gep.md), which calls `visit_gep_of_gep` when a
`getelementptr`'s source is one.
