> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/inferspace.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/inferspace.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/inferspace.rs sha256:00705838e64f3aa0a6c64aa60ee0378a72b1066f90ad66d619d72180b0d1e34a -->

# inferspace

Replace a far pointer made from a near one by the near one where its users
allow it.

## How it works
`addrspacecast` of a near pointer to a far space addresses what the near one
does, so a load, a store or a `gep` through it can use the near pointer and
drop the selector: no `les`, no segment register held across a loop.

```text
%w = addrspacecast ptr %p to ptr addrspace(1)      %q = gep i8, ptr %p, 4
%g = gep i8, ptr addrspace(1) %w, 4           =>   load i16, ptr %q
load i16, ptr addrspace(1) %g
```

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

## Runs
In every body's pipeline before `gvn`, so a far pointer cast from a near one
is read as the near one; part of the fixed point. `narrowspace` uses
`on_stack`.

## Keeps
Dominators and loops stay valid.

## See also
LLVM's InferAddressSpaces (InferAddressSpaces.cpp).
