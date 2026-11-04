> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/copyback.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/copyback.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/copyback.rs sha256:91c630f2ef2e50ed847604f28fdd84c4f44dc067101611f37765ae944b8a1979 -->

# copyback

A by-reference copy-out nothing can change: BASIC passes a far array's
element to a near formal through a slot, and stores the slot back after the
call.

## How it works
A store of the value just loaded from the same address, with one direct
call in between, is dropped when

- the call hands the callee a slot the value was copied into, in a parameter
  that is `readonly nocapture`, and
- the callee writes no memory but its own locals and further copy-outs of
  this kind, and calls only callees that do the same: a set found as the
  greatest fixpoint, so a recursive traversal belongs to it.

No write but a copy-out of an unchanged slot can then happen during the
call, so the element is what the load read and the store writes it back
unchanged: the result is BC's own, whatever the callee does.

## Example
```text
 %v = load i16, ptr addrspace(1) %e        %v = load i16, ptr addrspace(1) %e
 store i16 %v, ptr %slot                   store i16 %v, ptr %slot
 call @f(ptr %slot)  ; readonly nocapture  call @f(ptr %slot)
 store i16 %v, ptr addrspace(1) %e   ==>
```

The store is what kept a body that only reads its arguments
`memory(readwrite)`, so its descriptor pointer was reloaded after every call
and [`crate::argpromotion`](argpromotion.md) refused it.

## Runs
In the interprocedural step, before [`crate::argpromotion`](argpromotion.md), once the
callees' parameters are stated `readonly nocapture`; the memory effects are
stated again after. Not at -O0.
