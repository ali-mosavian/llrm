> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/framefree.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/framefree.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/framefree.rs sha256:59d0a080ad5b7d1bee565aa645d34d921580d50f84e76f600e1a19c56aa948a0 -->

# framefree

LLVM's `hasFP`, decided before allocation: whether a function can do without
its frame register, which is then one more value register (and callee-saved:
the prologue pushes it where it is used).

It can where `masm` can address every frame cell through the stack pointer,
which needs the stack's depth at every instruction. This reads the same
things `masm::stack_addressed` reads, from the LIR before allocation: spill
code adds cells and moves, no push or pop. What cannot be known here (a
stack pointer written another way, an indirect call that may pop, inline
code, a frame register read as a value) keeps the frame register.
