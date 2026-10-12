> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/counting.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/counting.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/counting.rs sha256:1a7ae4d90ab3797a15ff4506ec9529924dd41a8017a39004ab2dbc055ca35e07 -->

# counting

What a counted-loop rewrite places before the loop: its seeds, skip test
and exit values. Adapted from llrm-core's `optimize/counting.rs`, the
port of `qbopt/optimize/counting.py`; LLVM's counterpart is
SCEVExpander, as IndVarSimplify and LoopRotate use it.

Built only from `induction::CountedLoop`, so every pass that replaces a
loop's counter enters and leaves the loop the same way.

A seed is placed as it is made, before `Seeds::at`: the old ones were
ops held for the caller to splice once its proof was complete, so a
caller asks only once it will commit. `fresh` and `held` have no
counterpart: a value is its instruction's result, and a phi takes a
constant as it takes a value. `skip_guard` gives the compare; the
caller branches on it, as the old caller placed the branch it returned.
