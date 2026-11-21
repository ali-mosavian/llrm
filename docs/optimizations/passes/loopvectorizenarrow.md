> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopvectorizenarrow.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopvectorizenarrow.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopvectorizenarrow.rs sha256:7fd32a2717b271e631dce960ae8daebd577b0c47c4795306259e5c8a4fb690b1 -->

# loopvectorizenarrow

Narrow the integer chains a loop computes from its induction.

Translation: GCC `vect_determine_precisions` and
`vect_recog_over_widening_pattern` (`tree-vect-patterns.cc`): an operation
whose result needs fewer bits than its type is done at that width.
DEVIATE LV-NARROW-IV (owner llrm-target-x86-m64-b, exit 2026-11-30): the pin
has no such step; its `computeMinimumValueSizes` stops at a phi and has no
root here, so `(i & 15) + 1` of an `i64` counter is a `<8 x i64>` chain
(`pand`, `paddq`, then shuffles to the `i16` multiply). gcc keeps the
counter in `<8 x i16>`.

`and`, `or`, `xor`, `add` and `mul` of the counter and constants whose
result fits 16 or 32 bits become `sext (op (trunc counter), constants)`
(`zext` where the result fills the lane): the low bits of those operations
depend only on the low bits of their operands, and the result's upper bits
are zero. The `trunc` of the counter is what the vectorizer already widens
to a narrow `vec.ind`.
