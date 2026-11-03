> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_gep_base.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_gep_base.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_gep_base.rs sha256:719e2ad4bcf65b118f6c2b77e938946a536e003828e0f117314b9bfbc7c8d88f -->

# combine_gep_base

`CommonPointerBase` and `EmitGEPOffsets`: the pointer two GEP chains share
and the offset each adds to it.

Translation of
[`CommonPointerBase`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp#L2244)
and
[`InstCombinerImpl::EmitGEPOffsets`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L238)
at d1106deb71cc, and `OptimizePointerDifference`
([:2327](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp#L2327)).
`foldGEPICmp` and `visitSub` ask them.

MIRROR: the chains are walked from each side to the first pointer they
share, their flags are folded with `intersectForOffsetAdd`, a chain is
expensive past two multi-use GEPs after the first, and the offsets of
one-use GEPs are merged into the next multi-use GEP, which is rewritten to
reuse the sum.

DEVIATE 1: no vector GEPs: MIR has none.

DEVIATE 2: `OptimizePointerDifference` does not set `nuw` on the one GEP's
multiply or shift (:2336-2343). Exit: the `nuw` it would set is read by
no fold translated yet.

## Used by
`foldGEPICmp` (`combine_compares.rs`).

## Example
```text
%b = getelementptr inbounds nuw i8, ptr %p, i64 %x
%c = getelementptr inbounds i8, ptr %b, i64 -8
```
shares `%p` with `getelementptr i8, ptr %p, i64 24`; the offsets are `%x`
plus `-8`, and `24`.
