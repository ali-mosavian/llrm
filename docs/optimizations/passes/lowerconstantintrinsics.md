> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lowerconstantintrinsics.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lowerconstantintrinsics.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lowerconstantintrinsics.rs sha256:a01e971d2f4893949bd541000ace9c500ca129f4aad6f8c89977623f99b902ae -->

# lowerconstantintrinsics

`llvm.is.constant` made a constant: true where its argument is a manifest
constant at this point of the pipeline, false otherwise.

## How it works
Each `is.constant` call of a reachable block, in order, is replaced by its
answer and the users are simplified in turn
(`replaceAndRecursivelySimplify`). A conditional branch whose condition
becomes a constant takes its one edge, and the blocks that leaves
unreachable are deleted.

`llvm.objectsize` is the pin's second case and is not translated: llrm has
no `lowerObjectSizeCall` yet.

## Example
```text
%c = call i1 @llvm.is.constant.i16(i16 %x)    ->    br i1 false, ...   ->   br label %slow
```

## Runs
After Float2Int in the optimizing pipeline (slot 1597), and once more before
instruction selection at every level, as `PreISelIntrinsicLowering` does:
the backend has no `is.constant`.

## See also
LLVM's `lowerConstantIntrinsics`
([LowerConstantIntrinsics.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/LowerConstantIntrinsics.cpp#L101)).
