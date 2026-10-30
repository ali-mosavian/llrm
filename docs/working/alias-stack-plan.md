# Alias stack: the route to the pin's structure

Owner funcattrs-port (w14). 2026-10-25. Read before any code (coordinator). d_faces `-O2` m32, dist build of parity (profile at 796.9 M Ir; the ledger's 936.9 M is a later tip, so compare shares, not totals). Pin: `llc`/`clang` d1106deb71cc on the same file.

## Finding

1. The query side is not the cost. `BasicAA::alias_check` is 20.1 M inclusive; the pin's `BasicAAResult::aliasCheck` is 33.2 M, its `AAResults::alias` 30.5 M, `TypeBasedAAResult::alias` 5.7 M, `GlobalsAAResult::alias` 1.8 M. Ours is the cheaper stack.
2. The cost is analyses the pin does not have, run before any question is asked. The coordinator's 99 M is a sum of inclusive rows that nest (Writes asks CallFootprints asks PointerValues). Counted once, by own cost (inclusive less the other analyses it asks): **about 40 M**.
3. One row is not llrm-only at all and is 45x the pin: `GlobalsModRef::compute_with` 8.6-9.5 M inside `Summaries`, against the pin's `GlobalsAA::run` 0.20 M (once). `Sites::of` 5.1, `named_in` 2.0, `named_inside` 1.8 are our scans of every global's uses.

## Module by module (own Ir, M; calls from the profile)

| ours | own | what it is | pin counterpart | verdict |
|---|--:|---|---|---|
| `GlobalsModRef::compute_with` (in `Summaries`) | 8.6 | GlobalsAA: non-address-taken globals, per-function mod/ref | `GlobalsAAResult::analyzeModule` 0.20 | **at the pin by design, 45x its cost**: row S0 |
| `Summaries` (rest: `alias::summaries`) | 1.6 | per-body slices read and written (gcc IPA mod-ref) | FunctionAttrs `memory(...)` + GlobalsAA | pure deviation; consumers Stamp, settled, interprocedural |
| `PointerValues` (`alias::point_values`) | 7.0 | gcc `solve_graph` points-to over values | none (BasicAA walks `getUnderlyingObject` per query) | pure deviation |
| `CallFootprints` (`calls_by_attributes`) | 6.9 | what each call reads and writes, per call, by attributes | `AAResults::getModRefInfo(Call, Loc)`, asked when a walk needs it | same data as the pin, made eagerly; lazy |
| `Writes` | 0.1 | the stores of `CallFootprints` | none | goes with it |
| `Annotated` (+ `MemRef::of`) | 3.7 (1.7) | provenance on each load/store | `MemoryLocation` + TBAA metadata, asked | pure deviation |
| `BaseAccesses` (`Accesses::plain`) | 7.6 | a `MemRef` per load/store before the first question | `MemoryLocation::get` when asked | lazy: w3's #4719 (1.6 % of them are asked) |
| `ExposedFrames` | 3.3 | allocas whose address escapes | `PointerMayBeCaptured` per alloca, cached in `AAQueryInfo` | pure deviation, small |
| `Counted` (provenance side) | 3.8 | counted proofs for alias | SCEV | retire as SCEV covers clients |
| `MemorySsa::run` (`Mssa::build`) | 4.75 | the graph | `MemorySSA::buildMemorySSA` 2.9 | w3's S3 |
| `Accesses::run`, `PlainAccesses` carry | 0.8 | glue | | goes with the rest |
| **total** | **~40** | | | |

## Who still needs provenance (what blocks deleting `Annotated`/`PointerValues`)

`avail.rs` and `consts.rs` (through `fold`, `algebraic`, `floatfold`, `floatfacts`: w6's retirement list), `promote.rs`, `homes.rs` and `spares.rs` (off by default; delete), DSE (object of the pointer: root only, #4635), `earlycse.rs` (`Accesses::managed`), LICM (`PlainAccesses`, `Writes`, 1 site each). Everything on the MemorySSA path (walker, LICM motion, MemCpyOpt) already asks the stack pair by pair since 4f82e22e1.

## Stages (each: objsweep 0/568 or numbered rows, QCport `d_faces` object `cmp`, DIST Ir pair, `shadow_mssa` on the corpus before/after, `LLRM_CHECK_STALE`)

| stage | change | budget (M, own) | owner, date | blocked by |
|---|---|--:|---|---|
| S0 | GlobalsModRef: one pass over the module's uses per global kind, as `analyzeModule` does (`Sites::of` scans once) | -7.5 (8.6 -> ~1) | w14, 10-28 | none |
| S1 | MemorySSA gets a call's effect from `getMemoryEffects` (attributes + GlobalsAA) when it makes the access, and a call's location overlap lazily in `AliasQuery::get_mod_ref_info`; `CallFootprints`, `Writes` leave the MSSA path | -7.0 | w14 with w3, 11-03 | w3's S3 (shares `Accesses::of`) |
| S2 | `BaseAccesses` made when asked | -7.0 | w3 (#4719), 10-30 | none |
| S3 | provenance readers move to the pin's way one by one: DSE (`getUnderlyingObject`, `isIdentifiedObject`), EarlyCSE/LICM/MemCpyOpt on `BaseAccesses`; delete `homes`, `spares`; fold family leaves with w6; then delete `Annotated`, `PlainAccesses`, `PointerValues`, `Pointers` | -12 (3.7 + 1.7 + 7.0) | w14, per reader 11-06..11-20 | w6's fold/algebraic retirement |
| S4 | `ExposedFrames` -> capture of an alloca asked in the BasicAA query and cached per `AAQueryInfo`; `promote`/`cgpaddr` ask `capturetracking` | -3.3 | w14, 11-13 | S3 |
| S5 | `Counted` provenance side, `MemorySsa` build to 2.9 | -3.8 / -1.8 | w5 / w3 | SCEV clients |
| S6 | `Summaries` rest: Stamp/settled/interprocedural read function attributes (FunctionAttrs on simplified bodies in the walk, DEVIATE F6 exit) | -1.6 | w14, 11-20 | the stamp work (w6) |

Sum about -40 M of 797 (5 %); the pin's alias-side cost for the same IR is the 35-40 M of the query side we already undercut, so after S0-S6 the alias row is at the pin's.

## Risks

1. **Wrong output**: the b$seg / forged-selector rows (188fd3905, DEVIATE C1, AM-GA-1): BasicAA's `transparent`/`forged` decide the segment-pointer answers today, partly from `Annotated`'s provenance for the `restrict` roots. A noalias parameter is a restrict root only in provenance; the pin gets it from `noalias` argument attributes in BasicAA (`isIdentifiedFunctionLocal`) and `ScopedNoAlias`. S3 must keep both answers: the gate is a differential run of the old and new answers over every load/store pair of the corpus (the `shadow_mssa` tool does this for the graph) with every disagreement classified (precision lost, never a NoAlias gained).
2. **Precision**: provenance proves disjointness the pin cannot (gcc points-to). Each loss is a numbered AM row with objsweep bytes and clocks; the user's order (pin-faithful defaults first) accepts it.
3. **Overlap**: w3's #4719 (BaseAccesses) is S2 as written; S1 shares `Accesses::of(unit, refs, calls)` with w3's S3, so S1 starts after S3a/S3b land, to avoid editing the same constructor.
4. **Measurement**: the same-IR harness times whole passes (ours vs the pin on one IR); it has no row for an analysis, so the pin column here is the pin's own inclusive cost on its own IR for the same source. The pin has no analysis-side cost for BasicAA/TBAA: its numbers are the query side.

## Not in scope

The AA query side (`basicaa.rs`, `typebasedaa.rs`, `capturetracking.rs`, `aggregate.rs`) stays: it is the pin's structure and cheaper than the pin's.
