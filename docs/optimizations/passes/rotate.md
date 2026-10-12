> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/rotate.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/rotate.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/rotate.rs sha256:a77d4236f7917df5c888264062799757a9e3a79479cc50dca5e188c3329d5d2f -->

# rotate

Enter a loop proven to run at least once at its body, not at its test; and a
loop that is not proven, behind a copy of its test (gcc's `-ftree-ch`,
`tree-ssa-loop-ch.cc`): llrm-core's `optimize/rotate.rs`, the port of
`qbopt/optimize/rotate.py`, adapted to the rich MIR.
LLVM: LoopRotate, entering at the body where the first test is proven to
pass.

BC writes `FOR` as `jmp test; body: ...; test: cmp; jle body`. Where the
first test is proven to pass the entry goes straight to the body, and the
test is then reached only from the latch, which it merges into. `lsr`
enters a loop it counts to zero at its body too, behind a skip guard.

A rotated loop is no longer the pre-tested shape the counted-loop
analyses, peel and unroll read, so this goes after them.

What changed with the IR: the header's phis move to the body through
`ssa::SsaUpdater`, which also rewrites every use, outside the loop
included. Dropped: `_step_test` -- an `icmp` is a value, and reusing a
step's flags is isel's; the old body's `loop_trip_counts` and
`integer_ranges` side tables; clearing a private start (Dead's).
