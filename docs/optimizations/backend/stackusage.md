> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/stackusage.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/stackusage.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/stackusage.rs sha256:10b5d427173182df3ccce4819cf726dc149bdc41f69b786c007df6ac29439689 -->

# stackusage

How much stack a program may use, as GCC's `-fstack-usage` and
`-Wstack-usage`: each procedure's own frame, and the most a call from it
can reach through the procedures it calls.

A frame is what a call leaves on the stack: the return address, the
entry's pushes and the locals below BP, and the most a call's pushed
arguments hold at once. A routine the program does not define has a frame
this cannot know, so a bound through one is a lower bound, and a cycle of
calls has none.
