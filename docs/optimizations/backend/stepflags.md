> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/stepflags.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/stepflags.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/stepflags.rs sha256:d65b0d1fa9503b55a08bce589f3a4067aa5e796d3f8d5c4f44ff2e760c8ccd10 -->

# stepflags

A compare of a value with a constant that a step of the same value already
subtracts: the step's flags are the compare's.

`j1 := sub j, 1` and `cmp j, 1; jl` set the same flags, so the compare goes
and the step takes its place before the branch (X86's optimizeCompareInstr,
run before allocation as LLVM runs it). Left in, the compare reads `j`
after the step defined `j1`, so the two cannot share a register and the
counter is stepped through a `lea` and copied back.
