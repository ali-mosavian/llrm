# Old allocator modules: last callers and the PR that deletes each

Owner: splitkit-port. Rule: a module is deleted in the PR that moves its last caller. Checked 2026-10-23 on branch `delete-old` (callers outside the module, tests excluded). Objects: `=` identical expected, `~` changes, numbered as rows.

| Module | What is left | Last callers | PR (moves callers, deletes) | Objects |
|---|---|---|---|---|
| `allocate.rs` round allocator (`rewritten`, `_allocated`, rounds, `Facts`) | gone | none (0 of 9391 functions) | #3811 | = |
| `backend/splitkit.rs` | gone | only the round allocator | #3811 | = |
| `spiller.rs` analyses (`_stable_loads*`, `_constants`, `_addresses`, `_copies`, `_may_write`, `_keeps`, `_renamed`, `_widest`, `folded_source_*`) | ~1200 lines | ssaspill, ssarepair, constrain, storedhomes, peep/guards, loopslots, jumps/tailmerge, floatassign, frame, greedy/shadow | A: move to `backend/bodyfacts.rs` (branch `spiller-analyses` ready; opens when #3811 lands) | = |
| `spiller.rs` slot sharing (`slots_colored`, `_color_slots`, homes) | StackSlotColoring's driver | `allocate.rs::RegAlloc::run` | B: move into `slots.rs` (the pass that already holds `choose`) | = |
| `spiller.rs` emitter (`spilled_from`, `materialized`, `_folded_reads`, `_remove_abandoned`) | pre-RA read folding and spill | `allocate.rs::RegAlloc::run` (folded reads at `cpu.optimized`), `ssaspill.rs::spilled`, constrain tests | C: drop the pre-RA fold-and-spill (the pin folds inside `InlineSpiller`); deletes `spiller.rs` | ~ (numbered) |
| `ssaspill.rs` (SsaSpill pass, `Run`, `Prices`) and `ssarepair.rs` | llrm's own pre-RA spiller; the pin has none | `flow.rs` (pipeline stage), `assemble.rs` (`Run`), `target.rs`, `regclass.rs`, `regalloc_input.rs` | D: take the stage out of the pipeline; deletes both with C's callers | ~ (numbered) |
| `coalesce.rs` old part (`Graph`, `_interference*`, `_merged`, `_renamed`, `last_asked`) | the live `Coalescer` pass wrapper stays | `twoaddr.rs` (w-1's TwoAddress translation), `spiller.rs`, tests in `regalloc_fuzz_tests.rs` | E: after w-1's TwoAddress PR and C; deletes the old part | = |
| `spillforward.rs` | store-to-load forwarding for Peephole | `peephole.rs` (one call) | F: becomes a Peephole rule (`machinecp`'s neighbour); deletes the module | = |

C and D change what is spilled before allocation, so they come with a rows pair and numbered growth rows; they need w-1's PHIElimination (rows 59/61) only for the old-lowering test noted in #3811. A, B, E, F are moves. Order: #3811, A, B, C+D, F, E. No stacked PRs: each opens when the one before has landed.
