> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/calleefacts.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/calleefacts.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/calleefacts.rs sha256:d3cb0191cb74a66e8114df99b3e915696bb4166a20d9a9876aeaa6b4dc1907a1 -->

# calleefacts

What a function nothing outside the program reaches leaves different: the
registers its body, and the calls in it, write.

Such a function (`callgraph::direct_only`, in no cycle of calls) keeps its
convention, and each direct call of it clobbers what it wrote and did not
save, no more: a caller keeps a value in any other register across the call,
the scratch ones among them. (LLVM's no-CSR mode also drops the saves of
such a function, which hands the registers it then writes to every caller:
on nbody_fixed that cost more than the pushes.) The fact is made when
the function is machined, once, from its final LIR, and asked for by every
caller; the functions are machined callees first so it is there when asked.
LLVM's `RegUsageInfoCollector` and GCC's `-fipa-ra` do the same one function
at a time; this one asks the call graph for the order, so no caller meets a
callee whose fact is not yet made, and leaves a function in a cycle with its
convention.
