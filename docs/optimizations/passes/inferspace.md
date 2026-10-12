> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/inferspace.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/inferspace.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/inferspace.rs sha256:77c39dd96d2e124ffdaa87b13da5ef3b388bdae58687ac8e82f7485b5ba54883 -->

# inferspace

LLVM's InferAddressSpaces: a far pointer made from a near one by
`addrspacecast` addresses what the near one does, so what only reads or
writes through it, or steps it, can use the near one and drop the selector:
no `les`, no segment register held across a loop.

```text
%w = addrspacecast ptr %p to ptr addrspace(1)      %q = gep i8, ptr %p, 4
%g = gep i8, ptr addrspace(1) %w, 4           =>   load i16, ptr %q
load i16, ptr addrspace(1) %g
```

The near space a pointer narrows to follows where it points: DGROUP's for a
global, the stack segment's (the target's stack space) for a stack object,
whose far pointer is SS:offset and which a DGROUP pointer, read through DS,
would not reach unless SS were DS. No such assumption is made; a target that
states it could let the two spaces meet. Phis and selects of such pointers
follow, where every value they join is one, and nothing else reads them: a
far copy kept beside the near one for a call would cost a register. A cast
back to the near space, which the narrowing of a parameter leaves at each
call, is the near pointer itself.
