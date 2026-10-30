> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_castcall.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_castcall.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_castcall.rs sha256:288d6279e046e1b44eecc59e96f14a3e0ba96afb30074a2a071c0fddd2df756a -->

# combine_castcall

InstCombine's call through a mismatched prototype: a call whose function
type is not the callee's, to a function defined here, takes the callee's
type. Translated from `InstCombinerImpl::transformConstExprCastCall`
([InstCombineCalls.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCalls.cpp#L5313)).

## How it works
K&R `int none() {..}` called as `none(t)` is `call i16 @none(ptr byval(%S)
%t)`: the front end passes what the call site wrote. The callee takes
nothing, so the arguments it does not declare are dropped (the struct copy
feeding the `byval` goes dead with them), and a parameter the call lacks is
passed as null.

## Example
```text
call i16 (ptr) @none(ptr byval(%S) %tmp)   ; define i16 @none()
  ==>  call i16 @none()
```

MIRROR: the guards (a definition, no `thunk` or `naked` callee, no
`musttail` call, no `inalloca`/`preallocated` callee, an argument type for
each common parameter, `byval` agreeing on both sides, no `sret` among the
dropped arguments of a variadic callee), dropping extra arguments, null for
the missing ones, the call's calling convention and tail kind kept.

DEVIATE 1: the return type must be the callee's already (no cast of the
result), and a common argument's type must be the parameter's (no cast of
the argument): the pin bit-casts where the types are castable. DEVIATE 2:
calls only, not invokes (the combiner visits `Call`). DEVIATE 3: a variadic
callee keeps its extra arguments unpromoted, so one that would be promoted
(an integer narrower than `i32`) refuses.

## Used by
`combine_memintrinsics` (`visit_call`).
