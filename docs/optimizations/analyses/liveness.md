> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/liveness.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/liveness.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/liveness.rs sha256:8c63cf452ca84a137c1ba0c840a04fe5ff6e3726ae8c4eb5bd7ead490040f5dc -->

# liveness

Which SSA values are live at each block's entry and exit, and how many are
live at once.

## How it works
`live` returns `Liveness { live_in, live_out }`, keyed by `cfg::id`. It is
the least solution of

```text
 out(B)   = union of in(S) over successors S, plus the arms of S's live
            phis that come from B
 after(B) = out(B) - defs(B) + exposed(B)   (exposed: read before written)
 in(B)    = after(B) - phis(B)
```

Rather than iterate bit sets over every value, `solved` follows each value
from where it is read back to where it is made: a worklist of `Out(block,
value)` and `After(block, value)` facts. A value reaching its defining
block's ordinary operations stops there; one reaching its defining phi
continues along each predecessor edge with that edge's arm. The work is the
size of the live sets, not blocks times values. gcc's
`calculate_live_on_exit` and LLVM's `LiveVariables` walk from each use too.
A phi's arm is live out of the predecessor it comes from, not in the phi's
block.

`live_of` follows only the values it is given (a manager analysis brought up
to date for what a change named), from their reads, and gives their sets
alone. `ranges_of` gives the same by value, as sorted lists of blocks; the
update of the spill model's pressure edits only the values whose blocks moved
(of the values a pass names nearly all are read where they were).

```text
   B0:  %a = ...                 %a live_out(B0), live_in(B1) (read in B1)
   B1:  %p = phi [%a, B0], [%b, B2]    arm %a is live_out(B0) only
        ...                      %b live_out(B2)
   B2:  %b = ...
```

`live_where` solves for a chosen set of values only, plus the phis that take
them as arms, so a caller asking about a few values (the phis a pass takes
for cells) does not pay for the whole function. `entry_values` are the
values used and defined nowhere in the body: the parameters.

Pressure. `pressure` is the most values live at once, over the function or
over the blocks `inside`; `pressure_of` counts only values a predicate
accepts. It starts from `live_out` and walks each block backward.
Per-instruction views: `live_points` (live before and across each
instruction), `each_across` (the same over one set updated as it goes,
avoiding a copy per instruction), `live_steps` (the changes between
neighbouring instructions) and `pressure_points`. `phi_inputs` gives the
edge operands of phis whose results are live. `solves` counts the solves on
this thread, so a test can check that a caller reuses the one it holds.

Under `cfg(test)`, `live` is checked against a bit-set fixed point
(`live_dense`) on every function a test gives it.

## Example
A phi arm is live out of the predecessor it comes from:

```mir
define i16 @f(i16 %x) {
b0:
  %incoming = add i16 %x, 1
  br label %b1

b1:
  %merged = phi i16 [ %incoming, %b0 ]
  ret i16 %merged
}
```

`%incoming` is in `live_out(b0)`. With `ret void` in place of `ret i16
%merged` the phi is dead and `%incoming` is not in `live_out(b0)`.

From unit test `test_a_dead_phi_does_not_keep_its_edge_operand_live` in
`liveness.rs`.

## Provides
`Liveness`, `live`, `live_where`, `pressure`, `pressure_of`, `entry_values`,
`live_points`, `each_across`, `live_steps`, `pressure_points`, `phi_inputs`.
Consumers in llrm-transforms: `homes`, `spill`, `lsr`, `indvars`, `inline`.
The machine phases use `backend/liveness.rs`, over physical register lanes.

## See also
GCC `calculate_live_on_exit` (`tree-ssa-live.cc`); LLVM `LiveVariables`.
