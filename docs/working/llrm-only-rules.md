# llrm-only sites over 2 M: one rule each

d_faces -O2, parity 0aea1e46a, 936.9 M Ir. M is own Ir unless marked incl. Rules: DELETE (the pin does not do it), REPLACE (by a named pin pass), DEVIATE (keep, with the Ir saved against the object or clock cost). Owner rows are as the owner sent them; `pending` is asked and unanswered. Sources: [llrm-only-inventory](llrm-only-inventory.md), [residual-split](residual-split.md), alias-stack-plan (#4734).

## Alias stack (w14, #4734)

| site | rule | owner, when | M |
|---|---|---|---|
| Summaries: `Sites::of` computed 17x beyond the held one | REPLACE by the held GlobalsAAResult (pin: `GlobalsAA::run` once) | w14, S0 | -5.2 |
| Summaries, rest (`alias::summaries`) | REPLACE by FunctionAttrs `memory(...)` + GlobalsAA | w14, S6 | -1.6 |
| CallFootprints (6.9) + Writes (0.1) | REPLACE by `getMemoryEffects` (graph build, #4696); clients carry no call footprint (#4704). Measured. Askers outside MemorySSA (FloatFacts, ThroughMemory, `Accesses::managed`) uncounted | w3, both open | -9.65 |
| BaseAccesses (`Accesses::plain`) | REPLACE by `MemoryLocation::get` when asked: lazy accesses (design #4719, code 47a7369ed; clients ask 1.6% of references) | w3 | -7.5 (8.0 own) |
| `Accesses::run` glue + MemorySsa build (4.75 vs pin 2.9) | translate; `Mssa::build` 4.4 vs the pin's 2.97 is the floor, DEVIATE for the rest | w3 | -1.5 |
| PointerValues (`point_values`, 7.0) and Pointers | DELETE after its readers move; no pin counterpart (BasicAA uses `getUnderlyingObject`) | w14, S3; blocked by w6 retiring fold/algebraic/floatfold | -7.0 |
| Annotated + `MemRef::of` | DELETE with PointerValues (TBAA stays on the query side) | w14 | -5.4 |
| PlainAccesses carry | DELETE with Annotated | w14 | -0.8 |
| ExposedFrames (3.3) | REPLACE by `PointerMayBeCaptured` per alloca, cached in BasicAA; 2.1 leaves with lazy accesses, ~1.2 at S4 | w14 S4, w3 | -3.3 |
| Counted (provenance side, 3.8) | REPLACE by SCEV trip counts as clients move | w5 | -3.8 |
| Shape (6.0) | REPLACE by Dominators and Loops; w3 owns the MemorySSA ask (`shape.dominance` for `Mssa::build`), other askers uncounted | w3 for its ask; rest unowned | unknown |
| `Accesses::managed` (EarlyCSE 5.8, DSE 6.3) | DEVIATE until the provenance readers move (w14 S3); EarlyCSE on unannotated accesses was -2.63 but QCport +29 B, rejected | w14, w5 | 0 |

Query side (BasicAA, TBAA, CaptureTracking) stays: BasicAA `alias_check` is 20 M against the pin's 33. Gate for the DELETE rows: w14's never-a-NoAlias-gained differential (S0.5).

## Data structures (w4)

| site | rule | owner | M |
|---|---|---|---|
| `SlotIndex` stale compare (131 K compares, ~48 Ir) | REPLACE by the pin's entry reference (a number cell that is never freed, `!Send`; no thread spawns in non-test src) | w4, #4748 + #4756 | -10.8 measured (objects identical); 12 -> 16 bytes a `SlotIndex` |
| Vec growth (`finish_grow` 29.8 incl) | DEVIATE: long tail, no site above 0.3 M | none | 0 |
| `Function::successors`/`predecessors` Vec builds (7.9, 7.1 incl) | check callers first | w4 | ? |
| `Vec<Loc>::clone` (6.2, 44.9 K clones) | callers: `Semantics::clone` 5.1 (37 K), of which `Insn::clone` 2.0, `relayout::moved_body` 1.4, `masm::built` 0.7, `rewriter::_placed_for_rewrite` 0.65, isel `Selector` 0.5; the rest `Insn::clone` 0.7, `precse::shared` 0.2. Rule not set: the pin's operands are in place, a share of `Semantics` would make the clone a pointer copy | unowned | up to -5 |

## Passes and backend

| site | rule | owner | M |
|---|---|---|---|
| Decide@809 (19.1 in the d_faces -O2 ablation, 10.6 same-IR) | DELETE; needs #4731 and #4749 (both open), then the deletion PR. Cost: 14 of 568 vsgcc cells, quicksort 6 (+4..+48 B) and x_primes 8 (+0..+2 B) | w5 | -19.1 |
| Decide at the SimplifyCFG slots (15.8 same-IR vs the pin's 6.1; these are SimplifyCfg instances) | DEVIATE: no single lever (top step 10% of a slot); a loop-exits change is ~1.5 | w4 | -1.5 |
| ConstraintElimination (6.4 vs the pin's 3.9 same-IR) | DEVIATE: it is the pin pass, cost is diffuse; the sort comparator asks `fact_before` twice | w5 | -0.3, rest unknown |
| Dead (5.7), Interprocedural + Stamp (5.2 + 4.1), Counted/Followers (5.2) | unknown, needs a same-IR profile each; deleting Decide@809 takes ~27% of Counted and ~10% of Followers | none yet | ? |
| FloatAlloc + FloatPrep (14.8 + 7.8), MachineScheduler (10.7), PressureSink (4.6), MachineLICM (3.6), MachineCP (3.4), LiveRangeShrink (3.1) | pending | w2 | |
| Peephole `narrowed_arithmetic` 3.47 + `fused` 1.92 + `increments` 1.24 | REPLACE by the pin's isel patterns (narrowing, addressing-mode matching, UseIncDec); done as DELETE at the dag default flip, after a per-sub-pass zero count under `--isel=dag`; gated on w12's flip | w1, R45-3/4, 11-27 and 12-04 | -6.6 (upper bound) |
| Peephole `zeroes` 1.95 + `zero_compares` ~1.4 | same rule (MOV32r0, X86InstrCompiler.td); #4655 already makes dag use xor | w1, 11-27 | -3.3 |
| Peephole windows, the rest (14.3 of Peephole's 63.3) | REPLACE with the pin's single visit per instruction (R45-2, one walk) | w1, count 11-13, code 11-20 | up to -10 |
| Effects table 12.1 incl (`EffectKey::hashes` 4.5, `_effects_by_table` 6.2) | DEVIATE: no pin counterpart; cut the fill cost | w2 | unknown, up to -4.5; w2 to price |
| Flags liveness 4.3 (`_flags_dead_after`, `_flags_live_out`) | REPLACE FlagLive by `liveness::lanewise`, one liveness (spike: 1148 of 1156 blocks equal) | w5 | unknown until the PR |
| LiveRows 3.1 (`sets`, `live_rows`) | unknown: needs w5's liveness-reuse outcome | w5 or w13 | ? |
| `postrasink::sunk` 2.24 + `copysink` 1.16 | REPLACE by a translation of PostRAMachineSinking (pin 0.25) | w2, date unknown | -3.1 |
| `machinecp::propagated` 7-10 | w3 row 84 / w2 | w2, w3 | up to -6 |
| `masm::addressed_pass` 3.8 (`-fomit-frame-pointer` on assembly items, fixed point over string labels) | REPLACE by PEI `replaceFrameIndices` + X86FrameLowering (SPAdj before emission) | unowned; w1 only if assigned | up to -3.8 |
| FixupLEAs 2.1 | not llrm-only: the translated X86FixupLEAs | none | 0 |
| `allocate::DenseRows::sets` 7.2 and `live_rows` | REPLACE with LiveIntervals + LiveRegMatrix/LiveIntervalUnion (the pin's liveness for allocation); `live_rows` goes with it | w13 | up to -7.2 less the matrix cost, unmeasured; needs a split of who asks for `sets` |
| InterferenceCache `Cursor::move_to_block` 11.3 | none: the pin's `Entry::revalidate`/`update` at the pin's shape (w2 owns the file); open check: a caller asking one block twice | w13 | 0 |
| Kept / LaneLive asks | unknown: needs call counts per caller | w13 | ? |
| DAG builder and emitter sites over 2 M | pending | w9 | |
| D-I stable-load remat (`remat::values`, `bodyfacts::_stable_loads_*`), off by default | DEVIATE: costs 6.6, buys QCport -1.2..-1.6% bytes if on (stable-loads.md); exit 2026-11-13: translate as an invariant-load flag (X86InstrInfo's rule) or DELETE | w13 | -6.6 if deleted, QCport bytes +0.4% |
| LoopSlots 1.3 | DEVIATE for segment registers (ES set once before a string-move loop); REPLACE the rest by the pin's region split around the loop | w13 | up to -1.3, unknown until an objsweep with it off |
| SharedConstants 2.0 | REPLACE by MachineCSE (precse) once phi constant operands are candidates (#4699, #4700) | w13 | up to -2.0 |
| `constrain.rs` `constrained`/`required` | REPLACE by operand register classes and physreg-copy handling; needs the allocator to take class constraints | w13 | unknown, needs a profile of it alone |
| `shadow.rs` (`greedy_rewritten` harness) | DELETE after the dag default flip and Phase 6 | w12 flips, w13 deletes | tooling, 0 Ir |
| `lanes.rs` (128-bit lane mask) | DEVIATE: no pin counterpart for the flag lanes | w13 | unknown |
