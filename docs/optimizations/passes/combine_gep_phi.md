> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_gep_phi.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_gep_phi.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_gep_phi.rs sha256:0ed5968c203a4932cbbabeee7d8da1f8f71ff60b70703f735b6674082b3ae3c3 -->

# combine_gep_phi

`foldGEPOfPhi`: a `getelementptr` of a phi of `getelementptr`s as a
`getelementptr` of a phi of the one index they differ in.

Translation of
[`foldGEPOfPhi`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L3211)
at d1106deb71cc.

MIRROR: every incoming value is a GEP of one source type and operand count,
none is the GEP itself, they differ in at most one operand, which is not a
struct index past the first; the phi has one use if an index is to vary;
the new GEP has the flags all of them had.

## Example
```text
%p = phi ptr [ %a, %l ], [ %b, %r ]       %i = phi i64 [ %x, %l ], [ %y, %r ]
  (%a, %b: gep i32, ptr %base, i64 %x/%y)  %g = gep i32, ptr %base, i64 %i
```

## Used by
The `getelementptr` visit of [`combine_gep`](combine_gep.md), which
tries `fold_gep_of_phi` on a pointer that is a phi.
