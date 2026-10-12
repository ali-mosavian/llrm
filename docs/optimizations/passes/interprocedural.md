> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/interprocedural.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/interprocedural.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/interprocedural.rs sha256:0e7e7b981179d00148fa32d418cced5e87e25cc4f0e571dadedc5084c5aca3ce -->

# interprocedural

Adapted from llrm-core's `optimize/interprocedural.rs`: the whole-program
step a frontend runs once every body has reached its own fixed point:
inline, carry constants across direct calls, drop dead pure calls and
the tails of terminal ones, and send each changed body back through its
pipeline until no body changes.

A frontend hands in its pipeline; nothing here names a language or a
machine.

What changed with the IR: a call names its callee and carries its
actuals, so the old `Procedure`'s call, parameter, constant and argument
tables have no counterpart and a procedure is its module and
`GlobalId`; a procedure has every caller in the program when the program
does not export it, not by a parameter. A body changes in place, so the
pipeline takes the module, its analyses and the body's id. `propagated`,
the calls whose return was carried, has nothing to hold: a carried
return leaves the call's result unread. The old `Module` is `Proved`,
the name being llrm-mir's. `Interprocedural` is the step as a
`ProgramPass`.

The old module had no tests of its own.
