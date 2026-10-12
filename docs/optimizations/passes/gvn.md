> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/gvn.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/gvn.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/gvn.rs sha256:984c4e62697d744e1dce0ac1d4d9c31a03d8d761031cea4747ed5bbff4a2f54c -->

# gvn

Adapted from llrm-core's `optimize/gvn.rs`, the port of
`qbopt/optimize/gvn.py`: global value numbering and partial redundancy
elimination.

One pass owns reuse, whether the value came from arithmetic or memory.
`Gvn` is the pass; the old one sat with the pipeline in `transform.rs`.

What changed with the IR:
- A function argument is defined at the entry, ahead of everything.
- An inserted expression goes before its predecessor's unconditional `br`; a
  block leaves only by its terminator, so the old refusal of a predecessor
  holding a branch, return or escape is that terminator.
- The flags checks are gone: no value is the machine's flags. `Convert` (the
  register-pair extension) and `Copy` have no instruction, so what `joined`
  refuses of `_PURE` is the divisions, which may trap on a path that did not
  divide.
- `_on_edge` translates an instruction's operands; a phi's input may be a
  constant.

`floatfold::checks` has no counterpart: the rich MIR observes no FP
exception.

What each instruction touches is `memoryssa::Accesses`, asked once
before anything changes; without `Summaries` required, every call is
to an unknown callee.
`reused_divides` has no counterpart (see `transform`).

The old module had no tests of its own; `subexpressions`' are in
`transform` and these in `gvn_tests.rs`.
