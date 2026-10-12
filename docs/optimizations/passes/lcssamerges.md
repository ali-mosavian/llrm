> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lcssamerges.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lcssamerges.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lcssamerges.rs sha256:65ae5cbe919867586a7c649ecdbd0dea499416cabbabf90bfa91214bc2009e33 -->

# lcssamerges

Adapted from llrm-core's `optimize/lcssamerges.rs`, the port of
`qbopt/optimize/lcssamerges.py`: a loop with several exits is closed by
a phi at each exit, merged by phis where the exits' paths join.

Joins are resolved to the exit or join block that supplies them before
any phi is made, so a refused merge creates nothing. Python's
`ValueError`s are the `Err` text.

Ported tests are in `lcssamerges_tests.rs`. Stays behind, reading a BC
corpus: `test_compiled_early_exit_accumulator_is_closed`
(`lcmerge-p-g2.obj`; ignored there too).
