> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_select_to_cmp.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_select_to_cmp.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_select_to_cmp.rs sha256:762858865a2dfc013ebe98c2ac1d76248884baabaf8dd87091bcb1dd48990303 -->

# combine_select_to_cmp

`foldSelectToCmp`: a three-way compare written as selects, as `scmp`/`ucmp`.

Translation of
[`InstCombinerImpl::foldSelectToCmp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L4172)
at d1106deb71cc.

MIRROR: the six forms, in the pin's order, over integers.

DEVIATE 1: no vectors (MIR has none).

## Example
```text
%lt = icmp slt i32 %a, %b
%gt = icmp sgt i32 %a, %b
%z  = zext i1 %gt to i32                   %r = call i32 @llvm.scmp.i32.i32(i32 %a, i32 %b)
%r  = select i1 %lt, i32 -1, i32 %z   ==>
```

## Used by
Named by `combine_visit_select`.
