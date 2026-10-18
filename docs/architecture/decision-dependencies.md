---
architecture-index: a decision that records what it read
architecture-order: 120
---
# A decision that records what it read: the hoist forecast

TL;DR: a hoist price that was worked out and rejected is worked out again, identically, in the next round, because another pass changed the body somewhere. Recording the facts the price read and keeping it while they are unchanged is the right form, but the forecast reads every block, every value's total traffic and a greedy state that runs through the function in layout order, so no read-set that is both cheap to compare and exact exists. The ceiling is under 1% of the `d_faces` -Os compile. Closed as not kept; what a future version needs is listed at the end.

## What repeats

`d_faces` -Os, `LLRM_DEBUG=hoist,runs`, main `1a3e557c1`. Hoist's 30 idle steps cost 100.4 M own work; three of them (rounds 2, 3 and 4 of `d_draw_faces`: 5.8, 45.5 and 49.0 M) are 100.3 M of it. Each prices two runs with the whole-function forecast (`hoist::_affordable`: clone the function, `profit::weighted`, `profit::spill_forecast_after`) and lets both go. Among the 36 decision lines at -Os, 15 are the same line (`kept 3666176, moved 3666176 ...`); at -O2 the 29 are all different, because the run found there has no value crossing the loop and is never priced. The forecast (`spill::View::forecast`) is 2.52% of the -Os compile (hoist 1.83%, gvn 0.70%) and 1.39% of -O2's; the repeated share of hoist's is about 15 of 36, so an exact memo of it would save at most 0.8% of the -Os compile, not the 3.1% that the three steps' whole cost suggests (a step also pays `Accesses::managed` and the pressure it asks for).

## What the price reads

A hoist decision is `forecast(moved) - forecast(kept)` for a run, a loop and its preheader. `View::forecast` reads:

1. Every instruction of the function: `swept` walks each block in layout order and steps through its instructions with `liveness::live_steps`. A change in any block can change the cost.
2. The pressure of the function (`spill::Pressure`: liveness at each point, which values are counted, folded, addressed) and the cells that group values.
3. For each cell, its total traffic (`spill::traffic`): every use and definition in the function weighted by its block's frequency. One use added in another block changes the price of every cell it reads.
4. The block frequencies (`profit::_frequencies`), a function-wide estimate.
5. `Room` (registers, index scales, segments), the target's `OperationCosts`, and `kept_across` for each call.
6. A state that crosses blocks: `Sweep::spilled`, the cells given up so far, in layout order. A block's cost depends on which of its cells an earlier block already spilled.

Items 3 and 6 are what make a local argument unsound. The change of price from moving a run is local in the instructions it touches, but the cells it touches are priced by uses anywhere, and the greedy state that decides which are spilled is carried through every earlier block.

## Versions that exist

`Function::mark()` and `touched_since(mark)` give the blocks, values and edges edited since a mark; `Epochs` are four counters (cfg, memory, pointers, calls) a memo compares in O(1). Neither is a version of what the forecast reads: the epochs are too coarse (an arithmetic edit moves none of them and moves the price), and `Touched` is a digest per window, not a stamp per block or per value. keep-analyses' per-version design is the same scheme; no second one is proposed.

## A cheap and exact form would need

- a stamp per block (bumped when an instruction of it is added, removed, moved or rewritten) and per value (bumped when a use or a definition changes), so a record can hold `(block, stamp)` for the blocks it swept and `(cell, stamp)` for the cells it priced;
- the sweep checkpointed after each block (`Sweep::spilled` and the ranked sets), so a record valid up to block `b` resumes from `b`;
- the frequencies unchanged, compared by value: they change whenever a branch or loop is edited.

With these the forecast of `d_draw_faces` after an edit in block `b` is the prefix up to `b` reused and the rest swept again; a change in one of 200 blocks saves half the sweep on average, and a change to any cell's traffic invalidates every prefix block that holds the cell. The comparison work is the number of cells and blocks, which is most of what a sweep does. The gain is below the 0.5% bar and the machinery is a second version scheme in the spill model; closed.

## Oracle if it is ever built

`LLRM_CHECK_HOISTSKIP` already runs the skipped work and asserts it would have found nothing; a memo of the forecast would reprice and assert equal on the full corpus (`identical.py` with the check on), and a counted test would hold that a pass whose body changed outside the loop prices no run.
