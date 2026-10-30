> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/liveness.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/liveness.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/liveness.rs sha256:22d8e855a00de963ebebd7a3d4b9f310aee0415b19f20ff11e7c1d193b108eaf -->

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

`solved` is that fixed point on rows of bits, one row per block and a bit
for each value a block boundary can carry (a value read in a block it is
not made in, or taken by a phi): `out` is a union of the successors' rows
and `after` two word-wise operations, passes over the blocks from the last
until nothing changes. The work is the blocks times the words of a row times
the passes; following each value from its reads back to its definition (a
map lookup and a set insert for each value in each block it is live in, the
solver that stood here, kept as the oracle `LLRM_CHECK_LIVENESS` holds it
to) cost ~670 instructions a pair on `joins`. A phi's arm is live out of the
predecessor it comes from, not in the phi's block.

`live_of` follows only the values it is given (a manager analysis brought up
to date for what a change named), from their reads, and gives their sets
alone.

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
