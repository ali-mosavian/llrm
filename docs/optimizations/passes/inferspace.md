> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/inferspace.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/inferspace.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/inferspace.rs sha256:2aea7f2644346987cd8ebd6bee01bdef934bc3ca10c85860c7a45212a85ef23b -->

# inferspace

Replaces a far pointer made from a near one by the near one where its users
allow it, so an access needs no segment register loaded and held for it.

## How it works
`addrspacecast` of a near pointer to a far space addresses what the near one
does, so a load, a store or a `gep` through it can use the near pointer and
drop the selector: no `les`, no segment register held across a loop.

A far pointer that is a constant cast of a global (what forwarding a stored
far pointer leaves where its load was) is read the same way: a load or store
through it, or through steps of it that only memory reads, uses the global.

A far value narrows when every user is one that does not need the selector:
a non-volatile load's or store's pointer, the base of a `gep`, a phi or
select of narrowing values, or a cast back to the near space (which is the
near pointer itself). Anything else (a call argument, a compare, a stored
pointer) needs the selector, and a phi or select that another far value or
such a user also reads is left alone: a far copy kept beside the near one
would cost a register.

The near space a pointer narrows to follows where it points: the data space
for a global, the target's stack space for a stack object, whose far pointer
is SS:offset and which a data-space pointer read through DS would not
reach unless SS were DS. No such assumption is made. Phis and selects
narrow only where every value they join narrows to one space; a join of two
spaces stays far.

DEVIATE LP-5 (no pin pass on x86: infer-address-spaces is a target pass
(none for x86); the near pointer under a far cast matters at m16 only; exit:
deleted if the m16 rows do not lose without it, else folded into LP-4; owner
w-6; review 2026-11-13).

## Example
Before:

```mir
@g = global [64 x i16] zeroinitializer

define i16 @f(i16 %i) {
b0:
  %w = addrspacecast ptr @g to ptr addrspace(1)
  %a = getelementptr i8, ptr addrspace(1) %w, i16 %i
  %v = load i16, ptr addrspace(1) %a
  ret i16 %v
}
```

After:

```mir
define i16 @f(i16 %i) {
b0:
  %w = addrspacecast ptr @g to ptr addrspace(1)
  %0 = getelementptr i8, ptr @g, i16 %i
  %a = getelementptr i8, ptr addrspace(1) %w, i16 %i
  %v = load i16, ptr %0
  ret i16 %v
}
```

The load reads through a near `gep` of `@g`. The far `%w` and `%a` are left
for `dead`.

From unit test `a_load_through_a_step_of_a_cast_global_is_a_near_load` in
`inferspace_tests.rs`, whose module also declares a 16-bit data layout with
address space 1 far.

## Runs
In every body's pipeline before `gvn`, so a far pointer cast from a near one
is read as the near one; part of the fixed point. `narrowspace` uses
`on_stack`.

## Keeps
Dominators and loops stay valid.

## See also
LLVM's InferAddressSpaces (InferAddressSpaces.cpp).
