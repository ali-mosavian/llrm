> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_returned_arg.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_returned_arg.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_returned_arg.rs sha256:07ecb21edf82762b34f5a77ebcfdcfd111805550e67f23db0112d961ccacad8d -->

# combine_returned_arg

InstCombine's call that returns one of its arguments: the call's value is
that argument. Translated from the `getReturnedArgOperand` step of
`InstCombinerImpl::visitCallBase`
([InstCombineCalls.cpp#L5166](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineCalls.cpp#L5166)).

## How it works
A parameter marked `returned` (declared, or inferred by
`addArgumentReturnedAttrs`) comes back as the call's value, so every use of
the call reads the argument instead. The call stays: it may do more than
return.

## Example
```text
%r = call i32 @id(i32 returned %x)      ; use(%r)
  ==>  use(%x)
```

MIRROR: not a `musttail` call, the call has a use, the argument replaces
it.

DEVIATE 1: the argument must have the call's type; the pin also casts
where `canLosslesslyBitCastTo` holds. Reason: the verifier gives a
`returned` parameter its function's return type, so only a call through a
mismatched prototype differs, and `transformConstExprCastCall` takes those
first. Exit: none.

## Used by
`combine_memintrinsics` (`visit_call`).
