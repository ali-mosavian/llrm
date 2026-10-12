> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/jumpthread.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/jumpthread.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/jumpthread.rs sha256:89192d023004c1819d232c38ac81fa4a9fc25ea47d7f722ff553615f0b9e7f17 -->

# jumpthread

A switch in a loop that the paths into it decide, entered from the end of
each path at the case it decides: LLVM's `DFAJumpThreading` and GCC's
finite-state-machine threader (`tree-ssa-threadbackward.cc`).

`state = step(state); switch (state)` in a loop: the switch reads a phi of
the loop's header, and the value that comes round is a constant on each path
into the join that makes it (or constant from the preheader). Each such path
is given its own copy of the blocks from the join down to the switch, and
the copy of the switch is a jump to the case its constant picks: the
dispatch is gone from that path. The copies cost code, as LLVM's and GCC's
do, so a path is taken only where its blocks hold `MAX_PATH` instructions or
fewer and the whole function `MAX_COPIED`, and never where the build is
tuned for size, except the branch whose copies hold nothing.

Only what this reads: a `switch` on a phi in a block of a loop (`p` in `H`),
`H` reaching the switch's block `S` through blocks each with the one before
as its only predecessor, and `p`'s input from the loop being a constant,
or a phi in its own block `J` (a block ending in a jump to `H`) with a
constant from the predecessor `P`. The copies of `J..S` have `H`'s phis as
the values they take on the path; a value made in the originals and read
outside them is made again by the copies, and an updater (`SsaUpdater`)
joins the two where the paths meet. The result is not a natural loop any
more where several cases lead back: later loop passes skip it, as after
LLVM's.
