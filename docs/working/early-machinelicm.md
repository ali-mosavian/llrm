# Early MachineLICM, pressure-gated (codegen row 42) — w-3, plan note 2026-11-06

Premise: does the pin's `early-machinelicm` ([TargetPassConfig.cpp#L1368](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetPassConfig.cpp#L1368)) win anything over our output on the same IR?

## Test

`tools/pairs/mlicm.py`: our IR at the `switchconv` stage, to our back end and to the pin's `llc -O2` with and without `-disable-machine-licm`. Static instruction counts, in total and inside loops (a label to a later branch back to it; the loop-weighted column sums the ranges, so a nested loop counts per level). Checked first against its own failure: the pin's loops read 0 until labels starting with `.` were kept.

| corpus, level | all instr: ours / pin / pin without | in loops: ours / pin / pin without | loop-weighted: ours / pin / pin without |
|---|---|---|---|
| 71 bench programs m32 -O2 | 9800 / 8414 / 8399 | 4900 / 4183 / 4190 | 12181 / 8628 / 8650 |
| 71 bench programs m32 -Os | 5241 / 5048 / 5019 | 2592 / 2663 / 2675 | 3807 / 3889 / 3900 |
| 65 QCport modules m16 -O2 | 93219 / 88070 / 87886 | 49805 / 46865 / 47619 | 345734 / 338883 / 348514 |
| 65 QCport modules m16 -Os | 76531 / 74691 / 74326 | 38191 / 36701 / 36905 | 197982 / 218435 / 210623 |

## Reading

- m32: the pin's pass is worth 7 in-loop instructions of 4190 at -O2 and 12 of 2675 at -Os. No gap to close here; our in-loop count is above the pin's at -O2 for other reasons.
- m16 -O2 is the only place it shows: 754 fewer in-loop instructions (1.6%), 9631 loop-weighted (2.8%). Our in-loop count is 2940 above the pin's, so the pass is about a quarter of that gap. Per module it is mixed (`sbar.c` ours 1730 against the pin's 2252).
- m16 -Os: 204 fewer in loops (0.6%) and the loop-weighted count goes up (the pass hoists into the preheader what the loop then spills): the pin's gate does not make it a win there.
- The pass adds instructions overall (+184 at m16 -O2): it moves and copies, it does not delete.

## The pin's shape

`MachineLICMImpl::HoistOutOfLoop` ([L827](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineLICM.cpp#L827)) starts the pressure from the preheader (`InitRegPressure`, L933) and, per candidate, `IsProfitableToHoist` (L1273) asks `CanCauseHighRegPressure` (L1235): any register class whose cost is positive and whose pressure on any block of the back trace would reach the limit refuses the hoist; a cheap instruction is refused outright when it creates a loop-phi copy. We have no equivalent stage after selection: `licm` (mid-end) hoists IR, `loopslots.rs` hoists reloads.

## Attribution (2026-10-22)

`tools/pairs/mlicm_kinds.py`: the net in-loop instructions by kind, the pin's `llc` without the pass less with it, m16 -O2, per function. A text diff of the two listings reports a gross 1765 (register and label numbers differ throughout); the net over the five modules is 657.

| module | net | largest kinds |
|---|--:|---|
| mdl_ai | 247 | x87 `flds`/`fstps`/`fstp` 64, moves 21, loads 15 |
| ent | 223 | `pushl` 37, `addl` 29, loads 29, moves 23, `calll` 19 |
| cmd | 75 | `pushl` 15, moves 8, loads 8 |
| sc | 80 | loads 23, moves 21 |
| weapons | 32 | moves 9, loads 5 |
| all five | 657 | moves 82, loads 80, push/add-esp/call 136, x87 70, lea 23 |

No kind dominates. Constants and addresses (the kinds our mid-end `licm` could take earlier) are the smallest: `lea` 23, `mov $symbol` and `mov $imm` below that. Pushes, calls and `add esp` fall with the pass because a call sequence moved or a block was placed differently, which is layout, not hoisting.

## Decision

CG2 stays a DEVIATE with no exit planned: the residual is diffuse and 1.6% of in-loop instructions at m16 -O2 only. Reopen if a row shows a loop-invariant constant, address or load left in a loop.
