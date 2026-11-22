> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_stack.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_stack.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_stack.rs sha256:35e9c0ce745cb710cd353e425b3f46b6998ddd45981c539a1694ac3a6a4e6cac -->

# combine_stack

InstCombine's `llvm.stackrestore`: the removals of
`InstCombinerImpl::visitCallInst`
([InstCombineCalls.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCalls.cpp#L3725)).

## How it works
A restore is dropped when nothing could have moved the stack since its
save: the save is in the same block with no alloca, call with side effects
or restore of another save between them (the arrays were removed); when a
later restore in the block follows with no alloca or call between; or when
the block returns with none between the restore and the return. A save
nothing reads is then dead (`wouldInstructionBeTriviallyDead`).

## Example
```text
%sp = call ptr @llvm.stacksave.p0()      ==>   (nothing)
call void @llvm.stackrestore.p0(ptr %sp)
```

MIRROR: the three removals and their order.

## Used by
`combine` (its `visit_call`, the `llvm.stackrestore` intrinsic).
