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
| Shape (6.0 incl: 109 computes, 425 asks, 25 passes) | Split: `Dominators` 2.8 and `Loops` 1.3 are the pin's analyses at the pin's run count; Shape's own part is `natural` 1.9 (a `Forest::of_graph` walk per compute, 17.8 K Ir each) and the `Dominance` wrapper. REPLACE the readers by `Dominators`/`Loops` directly; w3 owns the MemorySSA ask (`shape.dominance` for `Mssa::build`) | w3 for its ask; the other readers (LoopSimplify 1.3, Fixed 1.1, LCSSA 1.1, GVN 0.46, Ports 0.43, Counted 0.4, LICM 0.38) unowned | up to -1.9 (`natural`), -4.1 stays |
| `Accesses::managed` (EarlyCSE 5.8, DSE 6.3) | DEVIATE until the provenance readers move (w14 S3); EarlyCSE on unannotated accesses was -2.63 but QCport +29 B, rejected | w14, w5 | 0 |

Query side (BasicAA, TBAA, CaptureTracking) stays: BasicAA `alias_check` is 20 M against the pin's 33. Gate for the DELETE rows: w14's never-a-NoAlias-gained differential (S0.5).

## Data structures (w4)

| site | rule | owner | M |
|---|---|---|---|
| `SlotIndex` stale compare (131 K compares, ~48 Ir) | REPLACE by the pin's entry reference (a number cell that is never freed, `!Send`; no thread spawns in non-test src) | w4, #4748 + #4756 | -10.8 measured (objects identical); 12 -> 16 bytes a `SlotIndex` |
| Vec growth (`finish_grow` 29.8 incl) | DEVIATE: long tail, no site above 0.3 M | none | 0 |
| `Function::successors` 7.9 incl (62.5 K calls, ~125 Ir) and `predecessors` 7.1 (40 K calls) | REPLACE the per-call Vec by a borrowed iterator, as the pin's `succ_begin`/`pred_begin` walk the use list. Top callers: successors `cfg::Block` build 1.1, `SemiNca::children` 1.0, loops 0.66, `condstores::merged` 0.47; predecessors `LazyValueInfoImpl::solve` 1.4, `iteratively_simplified` 0.6, `PromoteMem2Reg` 0.39 | w4 | up to -4, unmeasured |
| `BlockIter::next` 5.5 (310 K calls, ~18 Ir) | DEVIATE 0: the pin's list walk; 1.8 of it is `Vec<InstId>` collects (104 K calls) that a caller could skip | w4 | 0 |
| `Vec<Loc>::clone` (6.2, 44.9 K clones; -1.6 taken in #4762) | callers: `Semantics::clone` 5.1 (37 K), of which `Insn::clone` 2.0, `relayout::moved_body` 1.4, `masm::built` 0.7, `rewriter::_placed_for_rewrite` 0.65, isel `Selector` 0.5; the rest `Insn::clone` 0.7, `precse::shared` 0.2. Rule not set: the pin's operands are in place, a share of `Semantics` would make the clone a pointer copy | unowned | up to -5 |

## Passes and backend

| site | rule | owner | M |
|---|---|---|---|
| Decide@809 (9.3 on the dist pair, 779.71 -> 770.40, object identical on d_faces; 19.1 was a dev build; 10.6 same-IR) | DELETE; needs #4731 and #4749 (both open), then the deletion PR. Cost: 14 of 568 vsgcc cells, quicksort 6 (+4..+48 B) and x_primes 8 (+0..+2 B) | w5 | -9.3 |
| Decide at the SimplifyCFG slots (15.8 same-IR vs the pin's 6.1; these are SimplifyCfg instances) | DEVIATE: no single lever (top step 10% of a slot); a loop-exits change is ~1.5 | w4 | -1.5 |
| ConstraintElimination (6.4 vs the pin's 3.9 same-IR) | DEVIATE: it is the pin pass, cost is diffuse; the sort comparator asks `fact_before` twice | w5 | -0.3, rest unknown |
| Dead (5.7): `dead@677/768/801/837` | DELETE: 0 changes in 1561 runs each over the 568 vsgcc cells (`LLRM_DEBUG=runs`); `dead@1242` finds work (48 of 1480) and stays | w4, #4767 | -5.06 measured (objects identical) |
| Stamp 4.1 and Interprocedural 5.2 (`stamped_all` 6.8 incl, so the stamp is derived about twice: once in the slot, once inside the stages) | REPLACE `stamped` by FunctionAttrs (pin: PostOrder/ReversePostOrderFunctionAttrs, w14 S6); the stages by IPSCCP (`ipa<N>.`), ArgumentPromotion (`promote.`), and IPSCCP + FuncSpec (`ipa-cp.`). Module steps are not in the per-step `idle` table, so no never-fires count yet | w14 for the stamp; stages unowned | unknown until FunctionAttrs lands; one stamp instead of two is the first check |
| Counted/Followers (5.2) | REPLACE by SCEV trip counts as clients move; deleting Decide@809 takes ~27% of Counted and ~10% of Followers | w5 (offered a same-IR profile) | up to -5.2, unknown |
| MachineScheduler 10.7 (incl `graph` 2.4) | REPLACE by the pin's GenericScheduler pre-RA bottom-up (machine-scheduler-plan, 4 pieces) | w12 (handed over by w2) | unknown until piece 2 |
| PressureSink 4.6 | DELETE with scheduler piece 4 (11-06): the pin has no sink pass, the scheduler places loads. Gate: rows nothing worse | w12 | -4.6 |
| LiveRangeShrink 3.1 | KEEP, not llrm-only: the pin's pass (addPreRegAlloc, row 47), cost is the translation | w2 | 0 |
| MachineLICM 3.6 | REPLACE by the pin's early-machinelicm (row 42, pressure-gated) | w2 | unknown until it lands |
| MachineCP 3.4 | REPLACE by the pin's MachineCopyPropagation, two slots (row 41); cost is the second run | w2 | ~0 |
| FloatAlloc 14.8 | DEVIATE: no pin counterpart (the pin's x87 is FPStack, ours converts first); levers under 0.5 each | w2 | 0 |
| FloatPrep 7.8 -> ~3.5 after #4648 | DEVIATE, then DELETE when it moves into selection (floatprep-into-selection-plan) | w2, date unset | -3.5 |
| Register-effects table fill 12.1 | no rule yet: needs a same-IR profile | w2 | ? |
| Peephole `narrowed_arithmetic` 3.47 + `fused` 1.92 + `increments` 1.24 | REPLACE by the pin's isel patterns (narrowing, addressing-mode matching, UseIncDec); done as DELETE at the dag default flip, after a per-sub-pass zero count under `--isel=dag`; gated on w12's flip | w1, R45-3/4, 11-27 and 12-04 | -6.6 (upper bound) |
| Peephole `zero_compares` ~1.4 and `zeroes` 1.95 | `zero_compares`: DELETE at the flip (0 under dag). `zeroes` is the pin's post-RA MOV32r0 expansion (~1200 instructions per 65 modules under dag): DEVIATE, keep; the pin-shaped form moves the cost, saving ~0 until priced | w1, 11-27 | -1.4 to -2.6 (with `increments`) |
| Peephole windows, the rest (14.3 of Peephole's 63.3) | REPLACE with the pin's single visit per instruction (R45-2, one walk) | w1, count 11-13, code 11-20 | up to -10 |
| Effects table 12.1 incl (`EffectKey::hashes` 4.5, `_effects_by_table` 6.2) | DEVIATE: no pin counterpart; cut the fill cost | w2 | unknown, up to -4.5; w2 to price |
| Flags liveness 4.3 (`_flags_dead_after`, `_flags_live_out`) | REPLACE FlagLive by `liveness::lanewise`, one liveness (spike: 1148 of 1156 blocks equal) | w5 | unknown until the PR |
| LiveRows 3.1 (`sets`, `live_rows`) | unknown: needs w5's liveness-reuse outcome | w5 or w13 | ? |
| `postrasink::sunk` 2.24 + `copysink` 1.16 | REPLACE by a translation of PostRAMachineSinking (pin 0.25) | w2, date unknown | -3.1 |
| `machinecp::propagated` 7-10 | w3 row 84 / w2 | w2, w3 | up to -6 |
| `x86 tti::intrinsic_prices` (1057-line HashMap built on first use; the pin's TTI is static code) | DELETE, replaced by a bisect over the sorted text | w5, #4792 | -0.91 measured (d_faces -O2 758.17 -> 757.26) |
| `masm::addressed_pass` 3.8 (`-fomit-frame-pointer` on assembly items, fixed point over string labels) | REPLACE by PEI `replaceFrameIndices` + X86FrameLowering (SPAdj before emission) | w12 (coordinator) | up to -3.8 |
| FixupLEAs 2.1 | not llrm-only: the translated X86FixupLEAs | none | 0 |
| `allocate::DenseRows::sets` 7.2 and `live_rows` | REPLACE with LiveIntervals + LiveRegMatrix/LiveIntervalUnion (the pin's liveness for allocation); `live_rows` goes with it | w13 | up to -7.2 less the matrix cost, unmeasured; needs a split of who asks for `sets` |
| InterferenceCache `Cursor::move_to_block` 11.3 | none: the pin's `Entry::revalidate`/`update` at the pin's shape (w2 owns the file); open check: a caller asking one block twice | w13 | 0 |
| Kept / LaneLive asks | unknown: needs call counts per caller | w13 | ? |
| `DenseRows::sets` callers: `floatassign::prepared` 4.6, `peephole::addresses` 2.1, `fpstack::converted` 1.1 | exits-only live_out + trailing_zeros taken in #4747 (-3.4, objsweep 568 identical); the rest DEVIATE until w13's LiveIntervals lands (floatassign needs entry and exit sets), then REPLACE | w9, then w13 | -3.4 taken, up to -3.8 later |
| DAG builder and emitter sites over 2 M (combiner 3.4, `delete_dead` 3.2, CSE `hash_one` 2.9 + intern 2.0, schedule 2.3, TypeLegalizer 2.3, `Selector::select` 2.3, `live_users` 2.0 + iterator 2.0) | not llrm-only: each is the pin's translated pass; no rule, no cut owed | w9 | 0 |
| DAG CSE key (`Vec<EVT>`, `Vec<SDValue>` hashed as owned Vecs) | REPLACE by hash-of-parts without allocating, as FoldingSetNodeID hashes in place; after the DAG default flip (w12) | w9 | about -3 incl |
| D-I stable-load remat (`remat::values`, `bodyfacts::_stable_loads_*`), off by default | DEVIATE: costs 6.6, buys QCport -1.2..-1.6% bytes if on (stable-loads.md); exit 2026-11-13: translate as an invariant-load flag (X86InstrInfo's rule) or DELETE | w13 | -6.6 if deleted, QCport bytes +0.4% |
| LoopSlots 1.3 | DEVIATE for segment registers (ES set once before a string-move loop); REPLACE the rest by the pin's region split around the loop | w13 | up to -1.3, unknown until an objsweep with it off |
| SharedConstants 2.0 | REPLACE by MachineCSE (precse) once phi constant operands are candidates (#4699, #4700) | w13 | up to -2.0 |
| `constrain.rs` `constrained`/`required` | REPLACE by operand register classes and physreg-copy handling; needs the allocator to take class constraints | w13 | unknown, needs a profile of it alone |
| `shadow.rs` (`greedy_rewritten` harness) | DELETE after the dag default flip and Phase 6 | w12 flips, w13 deletes | tooling, 0 Ir |
| `lanes.rs` (128-bit lane mask) | DEVIATE: no pin counterpart for the flag lanes | w13 | unknown |
