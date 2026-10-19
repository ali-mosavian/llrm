---
architecture-index: the old ranges and regions modules: callers and deletion order
architecture-order: 520
---
# The old `ranges` and `regions` modules: callers and deletion order

TL;DR: `llrm_analysis::ranges` (2,772 lines) and `regions` (1,570 lines) are llrm's own interval and byte-range engines. Their callers move onto what the pin uses (LazyValueInfo, ScalarEvolution, ConstantRange, BasicAA and MemoryLocation, MemorySSA); each module goes when its caller count is zero. Dates are the owners' own, labeled as they gave them; a row without one says so.

Static count at `feat/parity` `42f21b907`, by `grep -rn "ranges::"` and `"regions::"` outside tests and the modules themselves. `tools/old_analysis_callers.py` counts analyses, not these modules (it reports `Ranges: 0 callers`).

## Callers

Replacement: what the pin asks instead. Date: the owner's stated date or condition; "estimate, X" is X's own estimate.

| Caller | File:line | Asks | Replacement | Owner | Date |
|---|---|---|---|---|---|
| decide threading engine | `decide.rs:192`, `:206` (`_threaded`, `_phi_threaded`, `_settled`) | LVI settling, threading | JumpThreading (translated) | vt-constantfolding | 2026-10-21, estimate, vt-constantfolding (needs the S4-only check on d_faces branch counts and a rows pair) |
| fixednarrow | `fixednarrow.rs:13`, `:158`, `:168`, `:195` | `scoped` bounds, operand, `product` | LVI `getConstantRange`, SCEV `getRange` | vt-constantfolding | 2026-10-23, estimate, vt-constantfolding (after Promote removal and the LVI-vs-SCEV table) |
| ports | `ports.rs:170-171` | operand intervals | LVI `getConstantRange` | vt-constantfolding | 2026-10-27, estimate, vt-constantfolding (same conditions) |
| `manager::Bounded`, `DominatedEdges` | `manager.rs:1993`, `:2065`, `:2105`, `:2200` | solved intervals, dominating edges | LVI, ConstraintElimination | vt-constantfolding | no date yet (after fixednarrow, ports, and the LVI API covering loop-carried values) |
| consts, floatbounds, guards | `consts.rs:722-726`, `floatbounds.rs:147`, `guards.rs:80`, `:99` | intervals, declared bounds | LVI, ConstraintElimination | vt-constantfolding | no date yet (same) |
| ranger | `ranger.rs:289`, `:323`, `:400`, `:566`, `:772`, `:880` | edge intervals, computed bounds | LVI | vt-constantfolding | no date yet (same) |
| parameter ranges | `parameter_ranges.rs:140`, `:158`, `:271` | operand and computed bounds | LVI `getConstantRange` | vt-constantfolding | no date yet (same) |
| indvars, exitfold, widen (via `Bounded`) | `indvars.rs`, exit fold | `Bounded` | SCEV `getRange` | scev-createscev-2239 | first reader PR 2026-10-20, estimate, scev-createscev-2239; the old widen path (`LLRM_NO_SCEV_WIDEN`) is deleted after it, no date |
| induction | `induction.rs:1815-1817` | `_computed`, `_operand` | SCEV `getRange` | scev-createscev-2239 | 2026-10-22, estimate, scev-createscev-2239 (after the trip-count readers: peel, unroll, loopsplit, the others) |
| alias bounds | `alias.rs:3921`, `:3958`, `:3964` | `bounded` for alias queries | BasicAA with LVI | basicaa-port-2163, blocked by funcattrs-port | after 2026-10-26, estimate, funcattrs-port (the old points-to tower goes after funcattrs-port's old call-effects path is deleted; basicaa gave no date of its own) |
| alias provenance | `alias.rs:4016`, `:4023`, `:4034`, `:4036` | `foreign_provenance`, `fixed_provenance` | BasicAA | basicaa-port-2163, blocked by funcattrs-port | after 2026-10-26, estimate, funcattrs-port (same tower) |
| memory `linear` | `memory.rs:15` | `regions::linear` | datalayout / `Spaces` | basicaa-port-2163 | first reader PR 2026-10-28, estimate, basicaa-port-2163; the F2 move itself no date |
| `classes_may_alias` | reached from `regions.rs` | alias classes | BasicAA | basicaa-port-2163 | after `regions.rs` stops calling it, mssa-green's DSE-on-MemorySSA moves off it, and funcattrs-port deletes the `alias::initialized` closure; no date |
| MemorySSA | `memoryssa.rs:142`, `:676`, `:697`, `:700`, `:803`, `:821`, `:1019`, `:1235`, `:1885-1898` | `overlapping`, `displaced_span`, `Frame`, `Index` | `MemoryLocation`, BasicAA, the clobber walker | mssa-green-2127 | 2026-10-29, estimate, mssa-green-2127 (after #2814) |
| avail, cellmap | `avail.rs:83`, `cellmap.rs:49` | `regions` imports, `ByteRange` | `MemoryLocation` | mssa-green-2127 | 2026-10-29, estimate, mssa-green-2127; the `avail.rs` overlap moves with the DSE swap, no date (waits on the `deadstore.rs` fixes, #2861) |
| loopidioms | `loopidioms.rs:1239` | `regions::overlapping` | BasicAA / `MemoryLocation` | loopidioms-rename-unroll-2238 | 2026-10-27, estimate, loopidioms-rename-unroll-2238 (after the non-trivial SimpleLoopUnswitch wiring PR) |
| old `Promote` | `promote.rs:29`, `:733-849` | overlap buckets, `overlapping` | none: `Promote` is removed | parity-pipeline | after #2840 |
| isel | `isel.rs:914-915`, `llrm-lir/lib.rs:136` | `scoped`, `exact_offsets_given` | KnownBits / ConstantRange address-mode match | lir-cleanup-port (asked, no reply) | no date given |
| homes | `homes.rs:255`, `:471` | `regions::overlapping` | BasicAA | unowned | no date given |
| floatloop, floatfacts | `floatloop.rs:327`, `floatfacts.rs:829` | `regions::overlapping` | BasicAA | unowned | no date given |
| tests | `llrm-x86-bc/tests/memory.rs:6`, `llrm-nib/src/test_borrowck.rs:1028`, `no_hand_solved_bounds.rs` | direct calls | move with their callers | whoever deletes the caller | after the callers |

The old `jumpthread.rs` calls no `ranges::` function. Registry ties from the coordinator's schedule (not owners' estimates): Followers and Counted deletes 2026-10-23, Registers 2026-10-24.

Translated pieces that exist now: LazyValueInfo (#2344, #2551), `ConstantRange`, ScalarEvolution, BasicAA, MemorySSA (#2516), GlobalsModRef (#2636). `FindAvailableLoadedValue` (`loads.rs`, mir-undef) is due 2026-10-26.

## Deletion order

1. The old `Promote` (#2840): drops the `promote.rs` rows.
2. `decide`'s threading engine, then the interval askers (`fixednarrow`, `ports`, `consts`, `floatbounds`, `guards`, `ranger`, `parameter_ranges`, `induction`, the indvars readers) move to LVI and SCEV.
3. `manager::Bounded` and `DominatedEdges` go; `alias.rs` and `memory.rs` leave `bounded` with the old points-to tower.
4. `isel` leaves `scoped` and `exact_offsets_given`.
5. The `regions` callers (`alias`, `memory`, `memoryssa`, `avail`, `cellmap`, `loopidioms`, `homes`, `floatloop`, `floatfacts`) move to `MemoryLocation` and BasicAA.
6. `ranges.rs` and `regions.rs` are deleted when `grep -rn "ranges::\|regions::"` outside the two files and their tests returns nothing.

## Callers without an owner

For the coordinator to assign: `homes.rs` (255, 471), `floatloop.rs` (327), `floatfacts.rs` (829). `isel.rs` awaits lir-cleanup-port's answer. funcattrs-port is the blocking owner of the points-to tower rows. Its estimates (each its own): the call-effects flip (attrs and GlobalsModRef) PR 2026-10-22; the old call-effects path deleted 2026-10-26; Stamp and Summaries deleted 2026-10-28, and Stamp's early run also waits for the W2 walk (cycles-queue), so that date may slip with it.

## Final deletion date

unknown: no date given for the `manager.rs`, `consts.rs`, `floatbounds.rs`, `guards.rs`, `ranger.rs` and `parameter_ranges.rs` rows (vt-constantfolding, before 2026-10-23), `classes_may_alias`, `isel`, `homes`, `floatloop` and `floatfacts`. The latest date given so far is 2026-10-29 (`memoryssa.rs`, `avail.rs`, `cellmap.rs`).
