> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/memloops.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/memloops.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/memloops.rs sha256:fc67bb3c346ecbda7b684c222c64dc78901182c0f2ffe6c2055d1f26a7e0e343 -->

# memloops

Turns the `llvm.memcpy` calls of chosen functions into byte loops, before
instruction selection.

## How it works
The pin's `PreISelIntrinsicLowering` expands a memory intrinsic into a loop
when the target has no routine for it (`createMemCpyLoopUnknownSize`). Here
the target picks the functions: it prices the function both ways, call and
loop, from its own backend's output, and expands the ones where the loop is
cheaper. A volatile copy stays a call. The rewrite is linear in the code it
adds.

DEVIATE: the loop steps both pointers and counts the length down, where the
pin's steps an index and leaves the pointers to loop strength reduction,
which has run by then. Exit: none.

DEVIATE: one choice for all the copies of a function, where the pin's is per
call; pricing each call apart is a compile of the function for each. Exit:
the allocator pricing a call by what it spills.

## Example
`call @llvm.memcpy(ptr %d, ptr %s, i16 %n, i1 false)`:

```text
  %z = icmp ne i16 %n, 0 ; br %z, loop, post
loop: %s1 = phi [%s, pre], [%s2, loop]   %d1 = phi [%d, pre], [%d2, loop]
      %k = phi [%n, pre], [%k2, loop]
      %v = load i8, ptr %s1 ; store i8 %v, ptr %d1
      %s2 = gep inbounds i8, %s1, 1 ; %d2 = gep inbounds i8, %d1, 1
      %k2 = sub %k, 1 ; %c = icmp ne %k2, 0 ; br %c, loop, post
```

## Runs
In a target's driver, on a copy of the module, for the functions it names.

## See also
LLVM's `createMemCpyLoopUnknownSize`
([LowerMemIntrinsics.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Utils/LowerMemIntrinsics.cpp#L35)).
