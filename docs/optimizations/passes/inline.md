> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/inline.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/inline.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/inline.rs sha256:759fe4967a5b2e8b1aeb1ba2b409640404c534b2b4fa405c88065b91e4bc1642 -->

# inline

Adapted from llrm-core's `optimize/inline.rs`, the port of
`qbopt/optimize/inline.py`: selective whole-module MIR inlining.

Inlining is a CFG operation, not a call peephole: split the caller at the
call, clone the callee's blocks, bind formal parameters to the actual
operands, and join every return back to the continuation.  The ordinary
body pipeline then simplifies the result.

A callee is a candidate when cloning it is safe; whether it is worth it is
cost: the body's priced work copied against the calls it removes, in the
target's clocks, or its code bytes at -Os, bounded by the `Threshold`
(nominal: a body's budget is at most 24 operations).  A
body that only reads or writes through its arguments or its own frame
inlines like any other: the clone keeps its memory operations and the
ordinary body pipeline turns the caller's argument cells into values.  A
public callee is inlined at its sites and stays defined; a private one
called nowhere else goes with its last site.  MIR chooses from semantic
costs and never sees opcodes or registers.  The call's price is profit's
`OperationCosts::call`.

What changed with the IR: a call names its callee and carries its
actuals, a formal is a parameter value, and the call's one result is what
`ret` returns, so the old call-site and ARG tables, formal entry cells
(`_parameter`), materialized actual copies, unmodelled extra call results
and width checks have no counterpart; the function type decides whether a
call fits its callee.  A candidate is a snapshot of its callee, as the old
one held the body it was chosen from.  A clone keeps its original's
metadata, which replaces merging the pointer and range side tables.  The
old `sealed` flag is a definition; a clone keeps its memory operations,
which replaces admitting only formal loads.  The clone is llrm-mir's
`splice`.  SSA is checked by the verifier
after each pass, not here.
