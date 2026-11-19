# LoopRotate keeps its analyses: port plan

Owner: object-refactor. Plan 2026-11-07; first PR by 11-14.

## Finding

`looprotate.rs` is already the pin's `LoopRotatePass` + `LoopRotationUtils`
(`rotate_loop` = `LoopRotate::rotateLoop`, `process_loop` = `processLoop`,
`simplify_loop_latch`, `should_speculate_instrs`, `update_branch_weights`,
`profitable_to_rotate_loop_exiting_latch`, `rewrite_uses_of_cloned_instructions`).
It runs in the pin's slots (`licm@724`, `rotate@728`, `licm@730`, `rotate@1621`).
What it lacks is deviation **R1**: no `DominatorTree`, `LoopInfo`, `SCEV` or
`MemorySSA` updates, so `run` returns `PreservedAnalyses::none()` after any
rotation and the next LICM rebuilds MemorySSA (w2's card: 64 of 65 builds).

`rotate.rs` (gcc `-ftree-ch`, `pipeline.rs:1407`, the tail slot) is a separate
non-pin pass for m16; it is out of this plan.

## Map: pin step -> what to update

| Pin (LoopRotationUtils.cpp) | Updater | Ours today |
|---|---|---|
| L705 `updateForClonedBlockIntoPred` | MSSAU | missing (w2) |
| L736 `MSSAU->applyUpdates(Updates, DT)` | DT + MSSAU | missing |
| L769/L787 `SplitCriticalEdge(DT, LI, MSSAU)` | DT, LI, MSSAU | `split_critical_edge`, no updates |
| L806 `MSSAU->removeEdge` | MSSAU | missing |
| L822 `MergeBlockIntoPredecessor(DTU, LI, MSSAU)` | DT, LI, MSSAU | `cfg::merge_block_into_predecessor`, no updates |
| `L->addBasicBlockToLoop` for new blocks | LI | derived again by the driver |
| SCEV `forgetLoop`/`forgetValue` | SE | `run.scev.take()` (drops all) |

Existing pieces: `DomTreeUpdater` (`llrm-mir/src/domtree_updater.rs`),
`LoopInfo::add_basic_block(s)_to_loop` / `remove_blocks_from_loop`,
`ScalarEvolution::forget_touched`, `mssa/updater.rs` (insert/move/remove).
Missing: `updateForClonedBlockIntoPred`, `removeEdge`, `applyUpdates` on MSSA (w2).

## PR chain (one PR per task, each output-identical, md5 over the corpus)

1. DT + LI: rotate through `DomTreeUpdater`/`LoopInfo` edits; `run` returns
   `Dominators` + `Loops` preserved; `rotate_loops` stops re-deriving `LoopInfo`.
2. SCEV: `forget_loop`/`forget_touched` in place of `take()`; `Scevs` preserved.
3. MemorySSA: with w2's updater entries; `MemorySsa` preserved.
4. Delete R1 and the "derive again" machinery.

Fail-first test per PR on the symptom: analyses survive a rotation (the
`made()`/build counters: no DT, LI, SE or MSSA build after `LoopRotate` when a
loop rotated), plus `verify` of each analysis against a fresh one.

## DEVIATE rows (exit dates)

| id | owner | what | exit |
|---|---|---|---|
| R1 | object-refactor | no analysis updates | PR 1-3, 2026-11-28 |
| R3 | object-refactor | new blocks at layout end (insert_block scans) | with PR 1 if layout insert is O(1), else stays |
| R5 | object-refactor | `simplifyInstruction` without DT/AC | PR 1 (DT then available) |
| R7 | object-refactor | check-exit-count makes its own SE session | PR 2 (shared SE) |
| R2, R4, R6 | - | option placement, no noalias scopes/coro, generic TTI | stay, documented |

## Costs to verify first (doubt the measurement)

Before PR 1: callgrind memexp -O2 for DT/LI/MSSA/SE builds attributable to the
analyses dropped by `LoopRotate` (not LICM's own), so the saving is priced before
the edit. LoopRotate itself is 22.9 M Ir (0.27%), 13 M above the pin.
