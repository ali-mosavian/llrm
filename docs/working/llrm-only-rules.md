# llrm-only sites over 2 M: one rule each

d_faces -O2, parity 0aea1e46a, 936.9 M Ir. M is own Ir unless marked incl. Rules: DELETE (the pin does not do it), REPLACE (by a named pin pass), DEVIATE (keep, with the Ir saved against the object or clock cost). Owner rows are as the owner sent them; `pending` is asked and unanswered. Sources: [llrm-only-inventory](llrm-only-inventory.md), [residual-split](residual-split.md), alias-stack-plan (#4734).

## Alias stack (w14, #4734)

| site | rule | owner, when | M |
|---|---|---|---|
| Summaries: `Sites::of` computed 17x beyond the held one | REPLACE by the held GlobalsAAResult (pin: `GlobalsAA::run` once) | w14, S0 | -5.2 |
| Summaries, rest (`alias::summaries`) | REPLACE by FunctionAttrs `memory(...)` + GlobalsAA | w14, S6 | -1.6 |
| CallFootprints (6.9) + Writes (0.1) | REPLACE by `getMemoryEffects` / `getModRefInfo(call, loc)` asked lazily | w3, #4696/#4704 | -7 |
| BaseAccesses (`Accesses::plain`) | REPLACE by `MemoryLocation::get` when asked | w3 | -7 |
| `Accesses::run` glue + MemorySsa build (4.75 vs pin 2.9) | translate | w3 | -1.8 |
| PointerValues (`point_values`, 7.0) and Pointers | DELETE after its readers move; no pin counterpart (BasicAA uses `getUnderlyingObject`) | w14, S3; blocked by w6 retiring fold/algebraic/floatfold | -7.0 |
| Annotated + `MemRef::of` | DELETE with PointerValues (TBAA stays on the query side) | w14 | -5.4 |
| PlainAccesses carry | DELETE with Annotated | w14 | -0.8 |
| ExposedFrames (3.3) | REPLACE by `PointerMayBeCaptured` per alloca, cached in BasicAA | w14, S4 | -3.3 |
| Counted (provenance side, 3.8) | REPLACE by SCEV trip counts as clients move | w5 | -3.8 |
| Shape (6.0) | unknown: readers vs Dominators and Loops not read | w3 or w5, pending | ? |

Query side (BasicAA, TBAA, CaptureTracking) stays: BasicAA `alias_check` is 20 M against the pin's 33. Gate for the DELETE rows: w14's never-a-NoAlias-gained differential (S0.5).

## Data structures (w4)

| site | rule | owner | M |
|---|---|---|---|
| `SlotIndex` stale compare (131 K compares, ~48 Ir) | REPLACE by the pin's entry pointer (`&'static` entry, `!Send`; no thread spawns in non-test src) | w4, after this table | -5 of 6 (0.8 taken, #4748); costs 12 -> 16 bytes a `SlotIndex`, to be measured |
| Vec growth (`finish_grow` 29.8 incl) | DEVIATE: long tail, no site above 0.3 M | none | 0 |
| `Function::successors`/`predecessors` Vec builds (7.9, 7.1 incl) | check callers first | w4 | ? |
| `Vec<Loc>::clone` (6.2) | check the clone sites | w1 | ? |

## Passes and backend

| site | rule | owner | M |
|---|---|---|---|
| Decide@809 (19.1 in the d_faces -O2 ablation, 10.6 same-IR) | DELETE; needs #4731 and #4749 (both open), then the deletion PR. Cost: 14 of 568 vsgcc cells, quicksort 6 (+4..+48 B) and x_primes 8 (+0..+2 B) | w5 | -19.1 |
| Decide at the SimplifyCFG slots (15.8 same-IR vs the pin's 6.1; these are SimplifyCfg instances) | DEVIATE: no single lever (top step 10% of a slot); a loop-exits change is ~1.5 | w4 | -1.5 |
| ConstraintElimination (6.4 vs the pin's 3.9 same-IR) | DEVIATE: it is the pin pass, cost is diffuse; the sort comparator asks `fact_before` twice | w5 | -0.3, rest unknown |
| Dead (5.7), Interprocedural + Stamp (5.2 + 4.1), Counted/Followers (5.2) | unknown, needs a same-IR profile each; deleting Decide@809 takes ~27% of Counted and ~10% of Followers | none yet | ? |
| FloatAlloc + FloatPrep (14.8 + 7.8), MachineScheduler (10.7), PressureSink (4.6), MachineLICM (3.6), MachineCP (3.4), LiveRangeShrink (3.1) | pending | w2 | |
| `masm::addressed_pass` (3.8), `backend::peep` windows, FixupLEAs (2.1) | pending | w1 | |
| InterferenceCache `move_to_block` (11.3), `DenseRows` (7.2), `live_rows` | pending | w13 | |
| DAG builder and emitter sites over 2 M | pending | w9 | |
