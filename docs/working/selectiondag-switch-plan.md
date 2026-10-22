# SelectionDAG switch plan, revised 2026-10-22

Replaces the dates of section 4 Phase 1 in [selectiondag-plan](selectiondag-plan.md) (switch "earliest 2027-02-04"). Dates are each group's own, given 2026-10-22; "honest" means a date the owner will sign, not a target. The switch gate is unchanged: zero compile failures and wrong outputs on the corpus (objsweep, QCport -Os, tests/run c/nib/qb/alias/huge, qbrt), no function refused, allocator stack switched first, full gate green. Size, clock and compile rows are numbered, not blocking.

## Where it stands (2026-10-22)

One scalar block runs end to end: builder, type legalizer, DAG legalizer, `select_node`, scheduler, emitter (`ret a+b`: `mov`, `add` with the load folded, `mov eax`). sdag-c's select-level shadow against the pin's `llc -stop-after=finalize-isel` (#3779): 3 of 160 functions equal; first causes: 80 stop at `BrCond` promote and 72 at `GlobalAddress` lowering (both legalizer, sdag-b), then calls (A2b/c), FP, wide ints. Selection's own gaps are not visible until those two land.

## What remains, by group

| Group | Remaining | Dates (owner's) |
|---|---|---|
| builder (sdag-a) | refuses now: va_*, dynamic alloca, atomics, EH, aggregates, freeze, intrinsics beyond mem*, switch with phi successors, float call results, i64 at m16. The gcc-corpus shadow (153 functions) hits none but memset.pattern (done); QCport, tests/run and qbrt are not yet run | byval/sret #3848 open; va_* 11-10; dynamic alloca 11-12; atomics 11-20; EH 11-27; aggregates, freeze, intrinsic sweep 12-04; **refuses nothing on the corpus 12-10** (shadow of QCport/tests/run/qbrt from 11-02) |
| legalizer, lowering (sdag-b) | i64 div/rem helper, remaining promote/expand (select, setcc, ext/trunc, neg, load/store, ret operands), x87 and FP conversions (SD6), bit counts, bswap, abs, funnel shifts | mul 10-27; div/rem 11-05; promote/expand 11-14; x87 11-18; bit counts 11-25; **legalizer complete 12-05 (types), 12-19 (lowering incl. x87)** |
| combiner (sdag-b) | first 8 visitors 12-12; known bits and SimplifyDemandedBits is the long pole | **complete for the scalar corpus 2027-02-12**; not a gate (rows, below) |
| matcher, m16 (sdag-c) | hand matchers (FrameIndex to LEA, MUL_LOHI, DIVREM, logic-immediate shrink) 11-05; x87 goldens 11-08; DAG segment overrides 11-10; m16 shadow against the old selector 11-14 | **no CannotSelect: m32 11-12, m16 11-20** |
| emitter, scheduler, harness (sdag-d) | below | below |
| corpus equality (sdag-d) | weekly shadow runs from the first whole-function path | below |

### sdag-d: emitter, scheduler, harness

| Piece | What | Date |
|---|---|---|
| D1d | blocks to `LirBody` and `Selected`: phis from `PHINodesToUpdate`, successors, block addresses, convention, frame; `--isel=dag` produces a function through `assemble` | 11-06 |
| D1e | emitter: local frame indices, globals, `CALLSEQ`, call clobbers (`RegisterMask`), jump tables, `EmitLoweredSelect` case 1/2, string ops, segment overrides (SD39, SD400) | 11-20 (calls after A2b/c) |
| D1f | scheduler: physical-register interference, `ClusterNodes`, vreg cycle (SD30), first run against the pin's order | 11-13 |
| D2a | shadow runs on the corpus, weekly (instructions, bytes, copies, executed, refusals) | from 11-09 |
| D2b | correctness: every refusal bucket named, owner told the same day; wrong output and compile failures first | from 11-09 |

## The earliest honest switch

| Gate | Date | Why |
|---|---|---|
| builder refuses nothing; legalizer complete for the scalar corpus | 2026-12-19 | sdag-b's lowering incl. x87 is the last (12-19); sdag-a's calls land well before |
| no CannotSelect, m32 | 2026-11-12 stated; **2026-12-19 in practice** | selection gaps appear only after legalizer buckets clear |
| emitter and scheduler complete for the corpus | 2026-12-04 | D1d..D1f above |
| two weekly shadow runs with zero wrong outputs, zero refusals | 2027-01-09 | D2; first clean run once the 12-19 gate holds, second a week later (holiday week counted) |
| allocator stack switched over | required; owned by splitkit | not on this critical path unless it slips |

**m32 default: 2027-01-14**, with the combiner still incomplete (quality rows numbered, not blocking, per the plan). **m16 default: 2027-02-04**: sdag-c's m16 table and shadow are done 11-20, but m16 needs far-pointer, segment and string-op emission (D1e) and its own two clean runs, with the old selector as the only oracle.

Combiner completion (2027-02-12) comes after the m32 switch; rows above main caused by a missing visitor are fixed by landing the visitor, not by keeping the old selector (Phase 2 deletions wait for those rows, so the old selector goes in 2027-03).

## What moves with it

The row-23 (CGP) functions blocked on "the DAG switch, earliest 2027-02-04" are blocked until **2027-01-14** at m32 and **2027-02-04** at m16.

## What would move these dates

- sdag-b's known-bits port sets the combiner date; the switch does not wait for it.
- Calls (A2b/c) are the biggest bucket after the two legalizer ones; if they slip past 11-27 the 12-19 gate slips with them.
- A shadow run that finds a new refusal bucket in the last week resets the clean-run count.
- The allocator stack: a switch date before it lands is not a date.
