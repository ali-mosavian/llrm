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
| `manager::Bounded`, `DominatedEdges` | `manager.rs:1993`, `:2065`, `:2105`, `:2200` | solved intervals, dominating edges | LVI, ConstraintElimination | vt-constantfolding | 2026-11-13, estimate, vt-constantfolding (the `DominatedEdges` and `Bounded` wrappers go after every consumer; registry rows: dominated-edges 2026-10-30, bounded 2026-11-06 if scev `getRange` lands by 2026-11-03, else it slips) |
| consts, floatbounds | `consts.rs:722-726`, `floatbounds.rs:147` | intervals, declared bounds | LVI, ConstraintElimination | vt-constantfolding and basicaa-port-2163 (both) | `consts.rs:722` and `floatbounds.rs:147`: 2026-11-06, estimate, vt-constantfolding (`consts.rs` is MemorySSA-fed, so mssa-green-2127 is a co-dependency: mssa may need to change something); 2026-11-10, estimate, basicaa-port-2163 (with `observers.rs`) |
| guards | `crates/opt/llrm-analysis/src/guards.rs:80`, `:99` | `ranges::declared` | LVI, ConstraintElimination | vt-constantfolding (confirmed; `declared` moves to ValueTracking and `ConstantRange`, `_implied_by_guards` to LVI and ConstraintElimination) | 2026-11-10, estimate, vt-constantfolding |
| ranger | `ranger.rs:289`, `:323`, `:400`, `:566`, `:772`, `:880` | edge intervals, computed bounds | LVI | vt-constantfolding | 2026-11-10, estimate, vt-constantfolding |
| parameter ranges | deleted (`parameter_ranges.rs`) | operand and computed bounds | IPSCCP (`ipsccp.rs`) | funcattrs-port | done 2026-10-24 |
| indvars, exitfold, widen (via `Bounded`) | `indvars.rs`, exit fold | `Bounded` | SCEV `getRange` | scev-createscev-2239 | first reader PR 2026-10-20, estimate, scev-createscev-2239; the old widen path (`LLRM_NO_SCEV_WIDEN`) is deleted after it, no date |
| induction | `induction.rs:1815-1817` | `_computed`, `_operand` | SCEV `getRange` | scev-createscev-2239 | 2026-10-22, estimate, scev-createscev-2239 (after the trip-count readers: peel, unroll, loopsplit, the others) |
| alias bounds | `alias.rs:3921`, `:3958`, `:3964` | `bounded` for alias queries | BasicAA with LVI | basicaa-port-2163, blocked by funcattrs-port | 2026-11-20, estimate, basicaa-port-2163 (`alias.rs` and the final MemRef delete); rests on funcattrs-port's old call-effects deletion 2026-10-26 and an unsurveyed `alias.rs` consumer set (Summaries, globalsaa, interprocedural, manager); basicaa-port-2163 restates it with the survey on 2026-10-31 |
| alias provenance | `alias.rs:4016`, `:4023`, `:4034`, `:4036` | `foreign_provenance`, `fixed_provenance` | BasicAA | basicaa-port-2163, blocked by funcattrs-port | 2026-11-20, estimate, basicaa-port-2163 (same tower; after 2026-10-26, estimate, funcattrs-port) |
| memory `linear` | `memory.rs:15` | `regions::linear` | datalayout / `Spaces` | basicaa-port-2163 | after F2's DataLayout step, 2026-11-10, estimate, basicaa-port-2163 (their part's final line) |
| `classes_may_alias` | reached from `regions.rs` | alias classes | BasicAA | basicaa-port-2163 | after `regions.rs` stops calling it, mssa-green's DSE-on-MemorySSA moves off it, and funcattrs-port deletes the `alias::initialized` closure; after 2026-10-30 (mssa-green) and 2026-10-26 (funcattrs-port) |
| MemorySSA | `memoryssa.rs:142`, `:676`, `:697`, `:700`, `:803`, `:821`, `:1019`, `:1235`, `:1885-1898` | `overlapping`, `displaced_span`, `Frame`, `Index` | `MemoryLocation`, BasicAA, the clobber walker | mssa-green-2127 | 2026-10-29, estimate, mssa-green-2127 (after #2814) |
| avail, cellmap | `avail.rs:83`, `cellmap.rs:49` | `regions` imports, `ByteRange` | `MemoryLocation` | mssa-green-2127 | 2026-10-30, estimate, mssa-green-2127 (DSE swap and the `avail.rs`/`cellmap.rs` callers); conditional on #2861 landing by 2026-10-28 (else 2 days after) and on basicaa-port-2163's F1 TBAA provider (8,968 pairs lose only on differing `!tbaa` ids) |
| loopidioms | `loopidioms.rs:1239` | `regions::overlapping` | BasicAA / `MemoryLocation` | loopidioms-rename-unroll-2238 | 2026-10-27, estimate, loopidioms-rename-unroll-2238 (after the non-trivial SimpleLoopUnswitch wiring PR) |
| old `Promote` | `promote.rs:29`, `:733-849` | overlap buckets, `overlapping` | none: `Promote` is removed | parity-pipeline | after #2840 |
| isel | `isel.rs:914-915`, `llrm-lir/lib.rs:136` | `scoped`, `exact_offsets_given` | `ValueFacts` table from the lowering (known bits, `ConstantRange`, LVI) | parity-pipeline (taken over from lir-cleanup-port) | moved to `ValueFacts` in #2966 (queued); after it lands only `fixednarrow.rs` and `nib/test_borrowck.rs` still call `scoped` |
| homes | `homes.rs:255`, `:471` | `regions::overlapping` | BasicAA | basicaa-port-2163 (to confirm for `homes.rs`; joins the MemRef survey #2931) | 2026-11-03, estimate, basicaa-port-2163 (with `floatfacts.rs`) |
| floatloop | `floatloop.rs:327` | `regions::overlapping` | BasicAA | basicaa-port-2163 (MemRef survey #2931) | first reader PR 2026-10-28, estimate, basicaa-port-2163 (with `slotstores.rs`) |
| floatfacts | `floatfacts.rs:829` | `regions::overlapping` | BasicAA | basicaa-port-2163 (MemRef survey #2931) | 2026-11-03, estimate, basicaa-port-2163 |
| tests | `llrm-bcobj/tests/memory.rs:6`, `llrm-nib/src/test_borrowck.rs:1028`, `no_hand_solved_bounds.rs` | direct calls | move with their callers | whoever deletes the caller | after the callers |

Not callers: `backend/allocate.rs`, `twoaddr.rs` and the other backend files that write `ranges::` import the live-interval module as `use crate::analysis::intervals::{self as ranges, ...}`; it is not this module. The old `jumpthread.rs` calls no `ranges::` function. Neither do `versionalias.rs` (scev-createscev-2239's LAA/LoopVersioning translation: first PR 2026-10-27, `versionalias` deleted 2026-11-05, estimates, scev-createscev-2239), `observers.rs` or `slotstores.rs`: grep finds no `ranges::` or `regions::` read in them, so they have no row; basicaa-port-2163's dates for them (`slotstores.rs` 2026-10-28, `observers.rs` 2026-11-10) are their MemRef moves. Registry ties from the coordinator's schedule (not owners' estimates): Followers and Counted deletes 2026-10-23, Registers 2026-10-24.

Translated pieces that exist now: LazyValueInfo (#2344, #2551), `ConstantRange`, ScalarEvolution, BasicAA, MemorySSA (#2516), GlobalsModRef (#2636). `FindAvailableLoadedValue` (`loads.rs`, mir-undef) is due 2026-10-26.

## Deletion order

1. The old `Promote` (#2840): drops the `promote.rs` rows.
2. `decide`'s threading engine, then the interval askers (`fixednarrow`, `ports`, `consts`, `floatbounds`, `guards`, `ranger`, `parameter_ranges`, `induction`, the indvars readers) move to LVI and SCEV.
3. `manager::Bounded` and `DominatedEdges` go; `alias.rs` and `memory.rs` leave `bounded` with the old points-to tower.
4. `isel` leaves `scoped` and `exact_offsets_given`.
5. The `regions` callers (`alias`, `memory`, `memoryssa`, `avail`, `cellmap`, `loopidioms`, `homes`, `floatloop`, `floatfacts`) move to `MemoryLocation` and BasicAA.
6. `ranges.rs` and `regions.rs` are deleted when `grep -rn "ranges::\|regions::"` outside the two files and their tests returns nothing.

## Callers without an owner

None. `homes.rs`, `floatloop.rs` and `floatfacts.rs` went to basicaa-port-2163 (`homes.rs` to confirm), `isel.rs` to lir-cleanup-port.

## Open dependencies

vt-constantfolding's dates are per file, from reading the call sites, and all depend on the LVI and SCEV API: scev `getRange` by 2026-11-03 (else `Bounded` slips), and the LVI-versus-SCEV table for `fixednarrow` (2026-10-23). Registry rows, same owner: float-facts, floating-point InstSimplify half 2026-11-06 (the MemorySSA half is mssa-green-2127's); `simplifyFCmpInst` 2026-10-28, after m16-narrow-66h-ops's `compare_in`. Neither is a `ranges::` caller row.

## Blocking estimates

funcattrs-port blocks the points-to tower rows. Its estimates (each its own): the call-effects flip (attrs and GlobalsModRef) PR 2026-10-22; the old call-effects path deleted 2026-10-26; Stamp and Summaries deleted 2026-10-28, and Stamp's early run also waits for the W2 walk (cycles-queue), so that date may slip with it. jumps-tailmerge-2241 owns no row here; no date given.

## Final deletion date

About 2026-11-20 (estimate, basicaa-port-2163: `alias.rs` and the final MemRef delete), the latest dated row. Conditional on: #2861 by 2026-10-28, basicaa-port-2163's F1 TBAA provider, funcattrs-port's old call-effects path deleted 2026-10-26, scev `getRange` by 2026-11-03, and basicaa-port-2163's `alias.rs` consumer survey on 2026-10-31 (may move it). The `manager.rs` rows (2026-11-13, estimate, vt-constantfolding) are the latest among the rest.

## Fork instructions

A fork never touches a worktree it did not create. It works in its own worktree under a private path (`wt-<task>` under the scratch directory) with a private target directory, deleted after.
