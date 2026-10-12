> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lcssa.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lcssa.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lcssa.rs sha256:be2b2290666a8643af04496ca29548df8746140309af77cc1d822ff93dca10dc -->

# lcssa

Adapted from llrm-core's `optimize/lcssa.rs`, the port of
`qbopt/optimize/lcssa.py`: every supported natural loop in closed SSA
form.

The old body was immutable, so a closing was a substitution applied to a
copy; here each use is rewritten in place. An exit phi takes its value's
name with `.lcssa`, as LLVM names it, where the old one kept the variable
and took the next version. The old MIR's flag values, never closed, have
no counterpart: a condition is an ordinary `i1` value.

Every old test is ported, in `lcssa_tests.rs`.
