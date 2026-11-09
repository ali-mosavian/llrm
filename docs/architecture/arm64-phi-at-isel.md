# arm64: the selector emits phis (ARM64-PHI-AT-ISEL)

Ruling (oversight, 2026-11-09): selection emits real phis, as the pin's ISel
emits `PHI`; the shared `PhiElimination` runs after it. No SSA reconstruction.
Owner llrm-arm64-lir (design, plan); implementation goes to a second arm64
session. Exit 2026-11-15 if the arm64-phi session starts by 2026-11-10 06:00Z, else
2026-11-17 (6 working days from the start).

## Measured today (-O1, 43 functions of the 24 programs, 2026-11-09)

- Both selectors lower phis while selecting: the DAG path (the default,
  `IselMode::Shadow`) in `dag/emit.rs` `phi_edges`, from the `PhiCopy {to, dst,
  src, float}` list of `dag/function.rs`; the hand selector (its fallback) in
  `isel/control.rs` `phi_copies`/`emit_copies`/`edge` and `isel/mod.rs`
  `phi_registers`.
- 184 virtual registers are defined more than once, all by `Copy`: the phi webs.
- 4 more in 3 functions (main x2, fmt x2): a post-index load/store
  (`ldr x, [b], #n`) that writes its base register. Those are not phis; they
  break SSA for the same reason. `ldst_update.rs` makes them.

## Design

1. `Op::Phi { dst: Reg, incoming: Vec<(Reg, String)> }` (value, predecessor
   label), first in its block, in `mfunc.rs` with its `defs_and_uses` rows.
2. Selectors stop copying on edges. A non-register incoming value (constant,
   symbol address) is materialized into a fresh register at the end of its
   predecessor, as the pin does; the phi names that register. Same order of
   phis as today (the copy order of `sequence_copies` depends on it).
3. `mfunc::lower_phis(&mut MFunc)`: today's edge logic moved out of the
   selectors unchanged (`sequence_copies`, one branch or an edge block
   `.L<fn>.e<n>`, copies before the first branch). It runs right after
   selection in `driver.rs::lower_function`, so every private pass sees the
   MFunc it sees now. Object bytes are identical by construction; the A/B proves
   the construction.
4. The producer maps `Op::Phi` to a `PHI` LIR instruction. The stage harness
   (`LLRM_ARM64_STAGES`) calls `to_lir` on the MFunc *before* `lower_phis`, so
   the 13 SSA stages get an SSA body. The object-bytes path never sees that
   form: it calls `lower_phis` first, so the harness's body is not what a real
   compile holds. Check: one executed program compiled with the harness on and
   off, object md5 equal and the program's output equal.
5. Writeback: `ldst_update.rs` gives the updated base a new register tied to the
   base (the pin's `$wback = LDRXpost %base`). Each writeback is then one
   def-use pair for the SSA stages: the load/store defines `%new` (its address
   operand reads `%old`), every later use of the base reads `%new`; `%old` is
   untouched. `verify_for(in_ssa)` then sees one definition per register. The
   allocator must give `%new` the register of `%old` (the tie), as it does for
   `MOVK`. Fail-first: a loop with `ldr x, [b], #8` verifies as SSA (today:
   "defined twice") and keeps its bytes.
6. Gap 3 later: the shared `PhiElimination` replaces `lower_phis`. It places
   copies differently (a register per phi, split edges by liveness), so that
   step moves bytes; it gets its own A/B and its moved cells listed.

## Acceptance (coordinator's)

- A/B of arm64 objects at -O1/-O2/-Os over every corpus cell that compiles: zero
  moved (bytes, clocks); both-fail cells counted as not tested.
- Fail-first: a loop-carried value needs a phi (the header holds an `Op::Phi`
  with the preheader and the latch as incoming; before: a copy in each).
- SSA stages run on arm64 with counts, none listed as a pass without a change
  count.

## Risks

- Aggregate and float phis (hand selector `Val::Agg`; DAG `float` flag).
- Edge-block order and numbering: labels are local, but block order feeds the
  private passes; `lower_phis` appends edge blocks in the same order.
- The shared SSA verifier may find more than the 4 writeback registers on
  -O2/-Os; step 4 measures that first.

## Estimate (not a promise)

Op::Phi and rows 0.5 d; DAG selector 1 d; hand selector 1 d; `lower_phis` and
the A/B 1 d; producer and harness 0.5 d; writeback 1 d; SSA stage run and
counts 1 d: 6 working days from 11-11, so 11-15 needs two sessions (selectors
in parallel with the producer and writeback) and has no slack.
