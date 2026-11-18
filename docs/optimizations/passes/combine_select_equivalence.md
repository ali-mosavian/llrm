> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_select_equivalence.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_select_equivalence.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_select_equivalence.rs sha256:3ffaae66613ebbbb53d51e01bacc4f25b38b89c7de9ccd88e6f8f8c442527b5d -->

# combine_select_equivalence

`foldSelectValueEquivalence`: `x == y ? f(x) : z` where the equality tells
one arm what the other computes.

Translation of
[`InstCombinerImpl::foldSelectValueEquivalence`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L1704)
at d1106deb71cc.

MIRROR: the three steps in the pin's order: substitute into the true arm,
substitute into the false arm without refinement while dropping the
poison-generating flags of what it simplified through, and the
`trunc nuw` bool form.

DEVIATE 1: integers only; `canReplacePointersIfEqual` is not translated
(`simplify_select.rs` DEVIATE).

## Example
```text
%c = icmp eq i32 %x, 2147483647
%a = add nsw i32 %x, 1                     %r = add i32 %x, 1
%r = select i1 %c, i32 -2147483648, i32 %a   ==>
```

## Used by
Named by `combine_visit_select`.
