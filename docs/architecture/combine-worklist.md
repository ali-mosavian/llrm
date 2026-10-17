# One combine pass with a worklist (plan step 3.2)

TL;DR: `fold`, `algebraic`, `phiopt`, `dead` and `decide`'s branch folding become one pass that visits each instruction once and re-queues the users and operands of every change, as LLVM's InstCombine does. Today they run in every round: over the 71 vsgcc programs at -O1/-O2/-Os their reruns after round 1 are 15.7% of the compile's own work and 14.7% is idle runs; `fold` changes the body on 2.6% of its reruns, `algebraic` on 2.2%, `decide` on 4.2%, `phiopt` on none. Without the repeat runs 14% of objects at most change (202 of 1,447), worth 6.5% of the compile.

## The invariant

One pipeline position per pass, and a pass that finds work after its own run does it in the run: the combine re-queues from the edit log (`Touched`, #2071 and #2075), not from the pipeline coming back. This is LLVM's [InstCombine worklist](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp) (`InstCombinerImpl::run`: seed in reverse program order, pop, delete when trivially dead and queue its operands, visit, queue the users and operands of what the visit made) with `LastRunTracking` for the cases where a run cannot find anything (here: `settled` and `reads()`).

## What moves, what does not

- Moves: the existing rules of `fold`, `algebraic` (the rules of `_rewritten`), `phiopt`, the branch folding of `decide`, and `dead`'s on-the-spot deletion. No new rules.
- Does not move: `decide`'s hoisting and speculation, `gvn`, `dse`, the loop passes. Their positions are stage 2 (plan 3.1).
- Facts reach the combine by declared reads (the contract), asked per candidate: `advances` for the loops that hold a masked source (#2086), ranges for one candidate at a time when the ranges rule is visited. No function-wide fact is built for the rewriting. The numbers behind this are in `compile-time-spec.md` §8 and §10 (73 M of algebraic's 101 M is set-up, 2.1 M of it repeated on an unchanged body, so keeping the facts is not the lever).

## Evidence for each choice

| Question | Measurement | Reading |
|---|---|---|
| Do the members need repeats? | `LLRM_ONCE=<pass>`, 1,447 objects: fold 80, decide 83, algebraic 39, phiopt 0 | at most 14%, and instructions -3.4%, -2.6%, -0.5% when run once |
| Are the repeats mostly idle? | 213 run traces: reruns after round 1 / changed: fold 1,056 / 27, algebraic 972 / 21, decide 1,056 / 44, phiopt 994 / 0 | 96% to 100% idle; 408 M of 2,781 M own work |
| What enables the changed ones? | passes that changed the body since the member's last run, per changed rerun: fold: dse 22, loopmotion 22, dead 19, fill 19, lcssa 17, gvn 12; algebraic: dead 19, lcssa 16, dse 12, loopmotion 12; decide: lcssa 37, tailrec 22, dse 22, fold 22, loopmotion 22 | real enablers: a loop pass or `dse` leaves something to fold. LLVM runs InstCombine after them too; the combine's position is after the loop group and `dse` |
| Which absorptions are workarounds? | `dead` changes on 539 of 961 reruns, 519 of them after `dse` | a workaround: `dse` leaves the operands of a store it removed. LLVM's [DSE](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/DeadStoreElimination.cpp) deletes them on the spot (`deleteDeadInstruction`). Not in this change; it moves with the combine's on-the-spot `dead` |

## The slice, and when to stop

1. Before code: an Opus read of this page.
2. Vertical slice: run the combine once, at the position after the loop group and `dse`, over the 202 objects whose output differs without repeats, and compare object bytes and executed work (rows against min(gcc, clang), worst row beside the geomean) and the pass work.
3. Kill: more than 25% of objects need combine repeats after the slice (14% measured), or any row gets worse that the repeats held. A miss follows the plan's contingency table and gets a `compile-time-spec.md` §8 entry.
4. Budgets as counted tests: runs of the pass per body (fold and algebraic together run 94 times on d_faces -O2, InstCombine 88; spec §10) and visits per IR instruction (51 for `algebraic` today, §10), and no function-wide fact built by the pass.

## Risks

- A rule that relied on the order of the old passes in a round (`fold` before `algebraic` before `decide`): the worklist order is reverse program order, and the rules' interplay is checked by identical.py on the 1,583 objects before any position moves.
- `decide` calls `invalidate(none)` mid-run today; inside the combine it must not (spec §3.1, no invalidate-all inside a pass).
