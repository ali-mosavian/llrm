> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_select_to_phi.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_select_to_phi.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_select_to_phi.rs sha256:d1dd279e610f5b16b9ea649c19daddbdb1573893c4925e002cf37bb87ff12c4a -->

# combine_select_to_phi

`foldSelectToPhi`: a select on a dominating branch's condition is a phi.

Translation of
[`foldSelectToPhi`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L2696)
and `foldSelectToPhiImpl` at d1106deb71cc.

MIRROR: the candidate blocks (the select's and its operands'), the
immediate dominator's branch on the condition or its `not`, edge
dominance, availability of each incoming value.

## Example
```text
b: br i1 %c, label %t, label %f                  m: %r = phi i16 [ -1, %t ], [ %i, %f ]
m: %r = select i1 %c, i16 -1, i16 %i   ==>
```

## Used by
Named by `combine_visit_select`.
