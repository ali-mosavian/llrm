> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/algebraic.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/algebraic.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/algebraic.rs sha256:a0de7876f73a8f84324542ea8ab97518409dfec56fceda562b6cdee0840846fb -->

# algebraic

Algebraic identities over integer values, as LLVM's InstCombine and, for
a loop-carried operand, Reassociate.

Adapted from llrm-core's `optimize/algebraic.rs`, the port of
`qbopt/optimize/algebraic.py`. Every rewrite holds modulo 2^width, so a
rewritten instruction drops `nsw`, `nuw`, `exact` and `disjoint`: a
reassociated sum may overflow where the original did not.

Gone with the old MIR:
- `wholephis`, `wholestores`, `_halved`, `_halves`, `_takes_halves`,
  `_extracted`, `_recombined` and a `Concat` of constants: a long split into
  word halves. Every value is whole here.
- `_forwarded_zero_tests`: `or x, x` for its x86 flags; `icmp` reads the
  value.
- `_product`: a widening multiply's unused high half; `mul` has one result.
- `_zero_difference` and `_copied_zero`: `sub 0, x` is the negation here,
  and nothing is a copy.
- `wanted` and `wide`: which flags and which halves were read.
- `_distributing`'s and `_mask_scaled`'s pairing: the old rules read a
  snapshot and rewrote one op each, so both ops of a pair had to agree to
  change. Here a rewrite replaces both at once, on the current function.
