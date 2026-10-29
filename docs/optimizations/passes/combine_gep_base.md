> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_gep_base.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_gep_base.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_gep_base.rs sha256:85fa2febdac8d9a2b4ffb0810567de9627ad9ca6668e7ed6fc22cf07c8682250 -->

# combine_gep_base

`CommonPointerBase` and `EmitGEPOffsets`: the pointer two GEP chains share
and the offset each adds to it.

Translation of
[`CommonPointerBase`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp#L2244)
and
[`InstCombinerImpl::EmitGEPOffsets`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L238)
at d1106deb71cc. `foldGEPICmp` asks them today; `OptimizePointerDifference`
will.

MIRROR: the chains are walked from each side to the first pointer they
share, their flags are folded with `intersectForOffsetAdd`, a chain is
expensive past two multi-use GEPs after the first, and the offsets of
one-use GEPs are merged into the next multi-use GEP, which is rewritten to
reuse the sum.

DEVIATE 1: no vector GEPs: MIR has none.

DEVIATE 2: `RewriteGEPs` is always true, the only way `foldGEPICmp` calls
it. Exit: `OptimizePointerDifference`, which passes false.

## Used by
`foldGEPICmp` (`combine_compares.rs`).

## Example
```text
%b = getelementptr inbounds nuw i8, ptr %p, i64 %x
%c = getelementptr inbounds i8, ptr %b, i64 -8
```
shares `%p` with `getelementptr i8, ptr %p, i64 24`; the offsets are `%x`
plus `-8`, and `24`.
