> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/canonical.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/canonical.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/canonical.rs sha256:01a6b97d05e1539a237640a5733075c906db6f0f12a4bcfbd9ee6334397619c8 -->

# canonical

Canonical forms, as LLVM's InstCombine puts them: compares with the
constant on the right, no neutral terms, and a zero test at the width of
what it tests.

Adapted from llrm-core's `optimize/canonical.rs`, the port of
`qbopt/optimize/canonical.py`. Folding makes `icmp C, v` whenever a
compare's left operand becomes known, and no machine compares an
immediate against a register in that order. The compare is swapped, its
predicate mirrored.

What had meaning only in the old MIR is gone: a compare there was a
subtraction whose flags each reader tested, so one read by other than a
test got its constant copied into a value instead; here the predicate is
the compare's own, and every compare can be swapped. A compare of two
constants is left to folding. A neutral term's raised source bytes, and
its flags read by a carry, have no counterpart either.
