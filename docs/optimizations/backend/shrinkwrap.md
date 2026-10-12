> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/shrinkwrap.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/shrinkwrap.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/shrinkwrap.rs sha256:de6a79c29f98e6d46fef3ac79f90fa64f9c70f93ae590aedfa475da35fd2921e -->

# shrinkwrap

LLVM's ShrinkWrap: the registers a procedure keeps for its caller (SI and
DI), and its frame (the stack reserve and the frame register), set up where
they are first needed instead of at every entry, and taken back only on the
returns that path reaches.

A procedure that leaves early (`if row == 7 { return 1 }`) before any of its
loop touches SI paid a push and a pop for it on every call. Each piece (a
register kept, the frame) is set up at the block that dominates every use of
it, as gcc's `shrink_wrap_separate` places each component, and taken back at
each return that block dominates; where a return can be reached both
through that block and around it, or the set-up would sit in a loop, the
piece is set up at the entry as before. One piece's entry use no longer
holds the others there: `paths(n, m)` named ECX at its entry, and paid the
frame and four more pushes for its `n == 0` answer.
