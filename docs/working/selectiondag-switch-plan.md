# SelectionDAG switch plan, revised 2026-10-22

Replaces the dates of Phase 1 in [selectiondag-plan](selectiondag-plan.md) (switch "earliest 2027-02-04"). Dates are each group's own, given 2026-10-22. The gate is unchanged: zero compile failures and wrong outputs on the corpus (objsweep, QCport -Os, tests/run c/nib/qb/alias/huge, qbrt), no function refused, two consecutive weekly shadow runs clean, allocator stack switched first, full gate green. Size, clock and compile rows are numbered, never blocking.

## Measured today

Select-level shadow (sdag-c, #3779), 160 functions of 71 programs, m32 i486, against the pin's `llc -stop-after=finalize-isel` on the same IR; our side is builder, combiner, both legalizers, `select_node`:

| Result | Parity tip + #3779 | + #3852, #3820, #3828, #3851 (later run, 159 functions) |
|---|---|---|
| equal to the pin | 74 | 73 |
| `BrCond: promote operand` (type legalizer) | 82 | 0 |
| differs: ours-only `AND8ri`, the promoted i1 branch condition (sdag-b) | 0 | 42 |
| unselected `SDIVREM`/`UDIVREM` (sdag-c, next) | hidden | 14 |
| `ConstantFP` libcall / `BrJT` libcall | hidden | 8 / 3 |
| `ZeroExtend`/`SignExtend` expand; unselected `Constant`; `DEC32r` (pin only) | hidden | 3 / 2 / 2 |
| intrinsic call (builder), singles, shadow crash | 1 | 1, 10, 1 |

The first column's single cause, the i1 branch condition, hid the layer under it; in the second it is gone as a stop and shows as 42 functions that differ by an `AND8ri` the pin does not emit (a promote that keeps a mask). The next layer is visible and small: 14 + 8 + 3 + 3 + 2 + 2 are named buckets with owners, 10 are singles. Rate so far: 40 SelectionDAG PRs merged on 2026-10-22; sdag-c fixes a newly visible bucket within two days; equal went from 4 to 74 in one change. Not yet measured: QCport, tests/run, qbrt (sdag-a, from 11-02) and m16 (the pin has no m16; the oracle is the old selector, needing a map from its LIR forms to pin opcodes, sdag-c after 10-28).

## What remains, by group

| Group | Remaining | Dates (owner's) |
|---|---|---|
| builder (sdag-a) | `va_*`, dynamic alloca, atomics, EH, aggregates and freeze, intrinsics beyond `mem*`, switch with phi successors, float call results, `i64` at m16 | byval/sret #3848 open; `va_*` 11-10; alloca 11-12; atomics 11-20; EH 11-27; sweep 12-04; **refuses nothing on the corpus: m32 2026-11-17, m16 2026-11-24** (corpus table below; tool `tools/dagisel/builder_corpus.py`, #3851) |
| legalizer, lowering (sdag-b) | i1 branch promote (the 82), i64 div/rem, remaining promote/expand, far ops beyond load, x87 and FP conversions (SD6), bit counts, bswap, abs, funnel shifts | div/rem 11-05; promote/expand 11-14; x87 11-18; bit counts 11-25; **types complete 12-05, lowering incl. x87 12-19** (stated; the pieces end 11-25) |
| combiner (sdag-b) | eight visitors 12-12; known bits and SimplifyDemandedBits | complete for the scalar corpus 2027-02-12; **not a gate**: a row above main is fixed by landing a visitor |
| matcher (sdag-c) | hand matchers (FrameIndex to LEA, MUL_LOHI, DIVREM, logic-immediate shrink); x87 goldens; DAG segment overrides; m16 shadow against the old selector | matchers 11-05; x87 11-08; segments 11-10; m16 shadow 11-14; **no CannotSelect: m32 11-12, m16 11-20** (its buckets are now visible, 14 + 2 + 2 of them sdag-c's own) |
| emitter, scheduler, harness (sdag-d) | blocks to `LirBody`/`Selected` (#3800 is the first block); locals, globals, `CALLSEQ`, call clobbers, jump tables, string ops, segment overrides; scheduler physical-register interference and clustering (SD30); weekly corpus runs | `LirBody` 11-06; scheduler 11-13; emitter complete 11-20 (calls after A2b/c); weekly runs from 11-09 |

## The switch

m32 and m16 run in parallel. The weekly shadow run (every Monday from 11-09) is the gate's clock.

| | m32 | m16 |
|---|---|---|
| groups done | builder 11-17, legalizer 11-18, matcher 11-12, emitter 11-20 | builder 11-24, plus `i64` at m16 (builder, undated), far ops beyond load (sdag-b, undated), matcher 11-20 |
| two clean weekly runs | 11-23 and 11-30 | 11-30 and 12-07, if far ops beyond load (sdag-b, undated) are dated by 11-14 |
| **default** | **2026-12-01** | **2026-12-08**, else **2027-01-08** |

Why not earlier: the first clean run needs the m32 builder (11-17, sdag-a), the legalizer (11-18, sdag-b) and the emitter (11-20, me); the first Monday after is 11-23, the second 11-30. Everything on m32 ends by 11-20, so 12-01 holds if nothing new appears in a run; the earlier November date (11-30 with runs 11-16/11-23) misses by one day on the builder.

sdag-b's signed 12-19 for the whole lowering (now 11-18 for the scalar corpus) is after the weekly runs, but the switch needs only the pieces the corpus hits. If the shadow shows x87 or bit counts in the corpus (QCport has float code), those gate the switch and m32 becomes **2026-12-14**. The pieces themselves are dated 11-18 and 11-25.

## What moves these dates

- The 42 `AND8ri` functions (sdag-b's i1 promote): equality, not correctness, but they hide any bucket behind them.
- sdag-a's corpus shadow on 11-02 and the `i64`-at-m16 date.
- Calls (A2b/c, then the emitter half): each week past 11-27 moves both switches a week.
- A new refusal bucket in a run week resets the clean-run count.
- The allocator stack: a switch date before it lands is not a date.

## Row 23 (CGP)

The seven functions blocked on "the DAG switch, earliest 2027-02-04" are blocked until 2026-12-01 (m32) and 2026-12-08 (m16; 2027-01-08 at worst).

## Critical path to 2026-12-01 (m32)

| Step | Owner | Deliverable | Date | Depends on |
|---|---|---|---|---|
| 1 | sdag-b | i1 branch promote (#3852), `and x,1` fold (#3855) land | 10-24 | the runner |
| 2 | sdag-a | corpus shadow of QCport, tests/run, qbrt: the real builder refusal list | 11-02 | 1 (so the shadow sees past BrCond) |
| 3 | sdag-d | blocks to `LirBody`/`Selected`; `--isel=dag` makes a function | 11-06 | #3800 landing |
| 4 | sdag-b | remaining promote/expand 11-14, x87 and FP conversions 11-18, bit counts 11-25 | 11-25 | sdag-a's FP builder |
| 5 | sdag-c | no CannotSelect, m32 | 11-12 (11-20 with x87) | 1, 4 |
| 6 | sdag-d | emitter: locals, globals, `CALLSEQ`, jump tables, call clobbers | 11-20 | sdag-a calls |
| 7 | sdag-a | builder refuses nothing on the corpus | **m32 11-17, m16 11-24** | 2 |
| 7b | sdag-d | **behaviour gate**: `--isel=dag` assembles and RUNS programs at m32: a call-free tests/run c case first 11-06 (with `LirBody`, step 3), tests/run c with calls 11-20 (step 6), then tests/run + qbrt all groups equal output to the old selector, incl. sdag-a's FPRound cases (`16777217.0` as f32, `0.1+0.2` as f64) | first 11-06, all equal 11-20 | 3, 6, 7 |
| 8 | sdag-d | two clean weekly runs (opcodes equal in the select shadow AND output equal in 7b) | 11-23, 11-30 | 7b |

The step that costs the slip against 11-02 is **7**, now measured: the corpus holds far/segment pointers (7829 uses in 465 programs), `invoke` 841 and `landingpad` 75 (QB), m16 multi-register arguments 128 and results 61, `i64` at m16 47, intrinsics (`llrm.ia16.window/code/asm` 23, libm about 45, min/max 15, `cttz`/`ctpop` 9, `va_start` 8), casts 28, float call results 12, `freeze` 6; atomics, dynamic alloca and aggregates are absent. sdag-a's per-target dates: m32 11-17 (`va_start` 11-10; intrinsics, freeze, memcpy-const sweep 11-13; FP call results 11-17; one nib inline-asm block excepted pending a custom-intrinsic decision), m16 11-24 (exception handling, far pointers, pairs, `i64` quads and the window/code intrinsics exist only in m16 QB/qbrt). Everything else finishes by 11-20.

The select shadow compares opcodes, not behaviour; 7b is the gate for behaviour and runs inside each weekly run. The `llrm.ia16.asm` intrinsics (nib at m32 1, m16 window/code) are one custom node `X86ISD::LLRM_ASM chain, bytes symbol, input registers, glue`: sdag-a builds it with the register contract as `CopyToReg`/`CopyFromReg`, sdag-c's hand case selects a pseudo, and the emitter writes the bytes verbatim (DEVIATE SD402, exit: the frontend emits the pin's `INLINEASM`); emitter side 11-13.

## File collisions

This week `builder.rs` was touched by 6 commits (981 lines, mostly sdag-a) and `tests.rs` by 17 (1430 lines, all four owners). Of the SelectionDAG PRs titled so, 21 are closed unmerged (several are consolidations such as #3710+#3721 into #3756 and #3763; their cause is not classified here), and #3851 is conflicting now. `tests.rs` is the shared file that costs: **the first PR is the split of `tests.rs` into one file per area** (`dag`, `builder`, `legalize_types`, `lowering`, `combiner`, `x86_lowering`). sdag-d writes it (mechanical, no test changes), all others leave `tests.rs` alone from its start until it lands (one day); 10-24. `builder.rs` stays: one owner touches it; its `builder/` submodules already carry the new areas (`memops`, `switch`).

## A November m32 switch

On the m32 corpus (C 124 programs, nib 30, qb 18 of 132, qbrt 30 of 124; the rest of qb/qbrt do not compile at m32) the builder's blockers are: cast FPRound 26 (round through memory, not elided), libm intrinsics 26, FP call result 6, `va_start` 4, `freeze` 1, `umin` 1, `memcpy` from constant data 1, `llrm.ia16.asm` in nib 1. A switch on **11-30 (run) / 12-01** needs the weekly runs on 11-23 and 11-30: everything on m32 done by 11-20 (builder 11-17, legalizer 11-18, emitter 11-20), three days of slack for FPRound-through-memory moving the 11-13 sweep. A switch a week earlier (11-24) needs the runs on 11-16 and 11-23, so everything by 11-13, which is what moves:

| Step | Now | Needed | Days | Who needs help |
|---|---|---|---|---|
| 7 builder clean | 11-17 | 11-10 (`va_start` 11-10, sweep 11-13 pulled in by moving the FP call results to sdag-b's x87 work) | -7 | sdag-a; the nib inline-asm block needs the custom-intrinsic decision now |
| 4 legalizer | 11-25 | 11-18 (types 11-14, x87 11-14, bit counts 11-18) | -7 | sdag-b: ConstantFP libcall and bit counts to sdag-c, who is idle until the shadow shows new buckets |
| 5 matcher | 11-12 / 11-20 | 11-12 | 0 | sdag-c |
| 6 emitter | 11-20 | 11-13 | -7 | sdag-d: calls half to sdag-a (the builder already knows the call sequence) |
| 3 `LirBody` | 11-06 | 11-06 | 0 | sdag-d |
| combiner | not a gate | div/rem and extend-of-load (sdag-d, 11-03) are needed for the corpus equality, not the switch | 0 | sdag-d |

Two limits are not about people: the build slots (two shared; one test cycle of one crate is 10 to 20 minutes behind other sessions' builds, so each owner gets about 30 cycles a day), and a refusal found in the last run week resets the clean-run count. The builder's tail is now measured and is in the corpus (far pointers 7829 uses, `invoke` 841): the m32 builder is 11-17, so the runs are 11-23 and 11-30 and the switch 12-01.
