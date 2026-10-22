# SelectionDAG switch plan, revised 2026-10-22

Replaces the dates of Phase 1 in [selectiondag-plan](selectiondag-plan.md) (switch "earliest 2027-02-04"). Dates are each group's own, given 2026-10-22. The gate is unchanged: zero compile failures and wrong outputs on the corpus (objsweep, QCport -Os, tests/run c/nib/qb/alias/huge, qbrt), no function refused, two consecutive weekly shadow runs clean, allocator stack switched first, full gate green. Size, clock and compile rows are numbered, never blocking.

## Measured today

Select-level shadow (sdag-c, #3779), 160 functions of 71 programs, m32 i486, against the pin's `llc -stop-after=finalize-isel` on the same IR; our side is builder, combiner, both legalizers, `select_node`:

| Result | Parity tip + #3779 | + open #3820, #3828, #3821 |
|---|---|---|
| equal to the pin | 74 | 74 |
| `BrCond: promote operand` (type legalizer) | 82 | 84 |
| `integer wider than a register` (builder) | 2 | 0 |
| `intrinsic call` (builder) | 1 | 1 |
| differs (ours-only `SUB32rr`) | 1 | 1 |

One cause, the i1 branch condition, hides the layer under it: the counts say nothing about calls, FP or the combiner until it lands. Rate so far: 40 SelectionDAG PRs merged on 2026-10-22; sdag-c fixes a newly visible bucket within two days; equal went from 4 to 74 in one change. Not yet measured: QCport, tests/run, qbrt (sdag-a, from 11-02) and m16 (the pin has no m16; the oracle is the old selector, needing a map from its LIR forms to pin opcodes, sdag-c after 10-28).

## What remains, by group

| Group | Remaining | Dates (owner's) |
|---|---|---|
| builder (sdag-a) | `va_*`, dynamic alloca, atomics, EH, aggregates and freeze, intrinsics beyond `mem*`, switch with phi successors, float call results, `i64` at m16 | byval/sret #3848 open; `va_*` 11-10; alloca 11-12; atomics 11-20; EH 11-27; sweep 12-04; **refuses nothing on the corpus 12-10** (corpus shadow from 11-02) |
| legalizer, lowering (sdag-b) | i1 branch promote (the 82), i64 div/rem, remaining promote/expand, far ops beyond load, x87 and FP conversions (SD6), bit counts, bswap, abs, funnel shifts | div/rem 11-05; promote/expand 11-14; x87 11-18; bit counts 11-25; **types complete 12-05, lowering incl. x87 12-19** (stated; the pieces end 11-25) |
| combiner (sdag-b) | eight visitors 12-12; known bits and SimplifyDemandedBits | complete for the scalar corpus 2027-02-12; **not a gate**: a row above main is fixed by landing a visitor |
| matcher (sdag-c) | hand matchers (FrameIndex to LEA, MUL_LOHI, DIVREM, logic-immediate shrink); x87 goldens; DAG segment overrides; m16 shadow against the old selector | matchers 11-05; x87 11-08; segments 11-10; m16 shadow 11-14; **no CannotSelect: m32 11-12, m16 11-20** (hidden today behind the 82) |
| emitter, scheduler, harness (sdag-d) | blocks to `LirBody`/`Selected` (#3800 is the first block); locals, globals, `CALLSEQ`, call clobbers, jump tables, string ops, segment overrides; scheduler physical-register interference and clustering (SD30); weekly corpus runs | `LirBody` 11-06; scheduler 11-13; emitter complete 11-20 (calls after A2b/c); weekly runs from 11-09 |

## The switch

m32 and m16 run in parallel. The weekly shadow run (every Monday from 11-09) is the gate's clock.

| | m32 | m16 |
|---|---|---|
| groups done | builder 12-10, matcher 11-12, emitter 11-20 | the same, plus `i64` at m16 (builder, undated), far ops beyond load (sdag-b, undated), matcher 11-20 |
| two clean weekly runs | 12-07 and 12-14 | same dates if the undated items are dated by 11-14 |
| **default** | **2026-12-14** | **2026-12-18** if those are dated by 11-14, else **2027-01-08** |

Why not 2026-12-04: the builder is done 12-10 and the corpus runs that say what it still refuses start 11-02; the first of the two clean runs needs a builder that refuses nothing. If the 11-02 run shows the 12-04 sweep items (aggregates, freeze, intrinsics) do not occur in the corpus, the builder date moves to 12-07 and a switch on 2026-12-04 becomes possible; the plan is updated that day.

sdag-b's signed 12-19 for the whole lowering is after 12-14, but the switch needs only the pieces the corpus hits. If the shadow shows x87 or bit counts in the corpus (QCport has float code), those gate the switch and m32 becomes **2026-12-21**. The pieces themselves are dated 11-18 and 11-25.

## What moves these dates

- The i1 branch promote (sdag-b): until it lands the shadow sees nothing under it.
- sdag-a's corpus shadow on 11-02 and the `i64`-at-m16 date.
- Calls (A2b/c, then the emitter half): each week past 11-27 moves both switches a week.
- A new refusal bucket in a run week resets the clean-run count.
- The allocator stack: a switch date before it lands is not a date.

## Row 23 (CGP)

The seven functions blocked on "the DAG switch, earliest 2027-02-04" are blocked until 2026-12-14 (m32) and 2026-12-18 (m16; 2027-01-08 at worst).
