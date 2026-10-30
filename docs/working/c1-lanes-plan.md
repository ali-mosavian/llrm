# C1: sub-register lanes in live intervals (splitkit; plan 2026-10-24, first PR 2026-11-17)

TL;DR: a value here is one lane; LLVM's coalescer, live intervals and SplitKit carry a lane mask per operand and a `SubRange` per lane set. This is the plan to translate that, gated on parity-green and the LIR data model (rows 59/61). w-1's `fixupsetcc` waits on step 1.

## What differs today
- `Held { value, width }` names a width per operand, the interval is one range per value (`liverange.rs` has no `SubRange`). Narrow reads/writes of a wider value (`%31.sub_16bit` in the printed MIR) are carried by widths tables and peeling: `distinct_classes`, `written_narrower` in the spiller, `Tables::widths`, the coalescer's refusal of copies between widths (DEVIATE C1, `JoinVals` asserts no lane conflict).
- The pin's pieces that are missing, by file: LiveInterval (`SubRange`, `refineSubRanges`, `computeSubRangeUndefs`), LiveIntervalCalc (`extendToUses`/`createDeadDefs` per operand lane mask, `constructMainRangeFromSubranges`), RegisterCoalescer (`joinSubRegRanges`, `mergeSubRangeInto`, `pruneSubRegValues`, `pruneMainSegments`, `taintExtent`, `computeWriteLanes`, `usesLanes`, `addUndefFlag`, `setUndefOnPrunedSubRegUses`, `copyValueUndefInPredecessors`), SplitKit/SplitEditor (lane-aware `defFromParent`, `enterIntvBefore`), InlineSpiller (spill of a sub-register access).

## Lanes
A lane is a byte of the value (lanes.rs counts bytes of the register file already): `sub_8bit` = byte 0, `sub_8bit_hi` = byte 1 (AH), `sub_16bit` = bytes 0-1, whole = bytes 0-3. An operand's mask comes from its `Held.width` and, for AH, a high-byte marker the data model adds.

## Steps (one PR each, fail-first test each, rows numbered)
1. **Data model** (with w-1, rows 59/61): the operand names a lane mask; `Held.width` stays as the size of the access. Nothing reads the mask yet; every operand is "whole" or the width-derived mask. 2026-11-17 (first PR).
2. **`SubRange` in `LiveInterval`** and the lane-aware `LiveIntervalCalc` (main range = union of subranges; a value only accessed whole has none). Test: a 32-bit value with a 16-bit partial write keeps the upper lanes live across it. 2026-11-24.
3. **Coalescer lane pieces** in the order of the table above; `JoinVals` loses its assertion; the refusal of copies between widths goes. 2026-12-01 to 12-08 (rows C1/C6 of docs/working/row63-66-plan.md).
4. **SplitKit and the spiller** for lanes (spill of a sub-register access; no widths tables). 2026-12-15.
5. Retire `distinct_classes`'s byte peel and the width tables once 2 to 4 hold (numbered DEVIATE exits).

## Risks and what is not known
- The cost: subranges multiply interval work for values accessed at two widths; x86-32 code has few (the pin pays it too). Measured only after step 2 on the ledger.
- High-byte registers (AH..DH) need the data model to say which byte an 8-bit operand names; today the register file decides at allocation.
- Dates after 11-17 are estimates; only 11-17 is committed.
