# EarlyCse snapshot: DEVIATE EC-1 (2026-10-25)

TL;DR: the pin mutates the function in place and keeps MemorySSA current with `MemorySSAUpdater`; ours forks the function and the context first. The fork costs about 6.8 M over the whole compile; no variant tried removes it for less than it saves.

## EC-1: what differs
`EarlyCse::run` (`crates/opt/llrm-transforms/src/earlycse.rs`) forks `Function` and clones `Context` before the walk. The walk mutates `unit.function` (flags, operands, `replace_value`, the deferred erase) while the view, MemorySSA and the simplifier read the pre-walk body, so one body cannot serve both under the borrow rules. Erased stores stay defs in MemorySSA, which is only conservative.

## Cost
- 15 of 22 runs on the corpus change nothing; with `memssa=true` 10 of 11 change nothing. The snapshot of those is pure cost.
- Whole-compile snapshot cost: about 6.8 M.

## Tried
- Unannotated accesses from the graph (w-3 `c3e34bbeb`, same change as `ee31b1123`): d_faces -2.63 M, object identical, objsweep 0/568, but QCport 11/260 modules change, net +29 B, mixed. Unannotated accesses lose `is_use_trivially_optimizable_to_live_on_entry` (it reads `loc.reference.unwritable()`), so constant-memory loads are no longer forwarded. Fixing it through `pointsToConstantMemory` costs +2.1 M elsewhere. Not merged.

## Design left (dry run)
Fork lazily: take the snapshot at the first mutation, not at entry, so the 15 of 22 no-change runs pay nothing. Estimated net about 2 M. Needs the walk's mutation sites (`set_flags`, `set_operand`, `replace_value`, the deferred erase) to go through one `before_first_change` hook, and readers to hold the snapshot, not the live body. Not started: below the bar on its own.

Exit: review when MemorySSA gets an updater (w-3), which removes the need for the snapshot.
