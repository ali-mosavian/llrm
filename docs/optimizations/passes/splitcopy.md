> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/splitcopy.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/splitcopy.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/splitcopy.rs sha256:2f57c57b6fc560551391e587080d849c7e6580fa0edf628a9cd8a3b8e30f9c49 -->

# splitcopy

A `memcpy` into a local nothing else can reach is the loads and stores of
the bytes the local is read as. LLVM's counterpart is SROA slicing an
alloca's memcpys (`AllocaSlices`, `visitMemTransferInst`). Adapted from
llrm-core's `_split_copies`, which split one aggregate load and store
into the scalar leaves its destination already had; that pass has no
tests to port, and these are new.

What changed with the IR:
- The copy is a `llvm.memcpy` of a constant length between two exact
  pointers into frame objects, not an adjacent load and store of one
  aggregate.
- The leaves are the destination's loads. A byte nothing reads is not
  copied: the destination is private, so no one else can read it.
- A destination the copy chain reads again is split once the later copy has
  been, so a copy of a copy is the loads of the first source.

Left whole: a destination that is passed, stored, compared or read by
another memcpy; a volatile copy; loads that overlap in part.
