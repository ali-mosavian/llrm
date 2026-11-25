# m68k trial clock estimate: a layout-dependent weight (scoping, no code, 2026-11-24)

Class: any m68k trial verdict (`UNROLL-OS-TRIAL`, hint/loop inline trials, the per-function trials in `compile.rs`) is priced with `trial::price_of`, whose clock weights come from the layout of the lowered code, not from the CFG. A change that moves no bytes can move a verdict.

## (1) The doubt: why does a pass that changes no bytes change one function's loops 3x
- `price_of(items)` weights each instruction by `run_weights`: the share of runs reaching it (forward branches split 50/50) times 10 per loop it is in, at most 3 (`trial.rs`). A loop is a backward branch to a label; its members are the items in the layout span `[header, last latch]` that can reach a latch inside that span (`loop_members`).
- So membership and nesting depend on the order the blocks are placed in, and a hot block inside or outside one of several large overlapping spans changes by x10.
- Measured on the function behind M68K-ADDR-WIDE-68000 (unrolled build, 7,484 B in both): 47 loops in both builds, the same shapes, label numbers shifted by 1-2, member counts differing by 1-3 per loop (three large ones: 270 / 248 / 10 members against 267 / 246 / 10). Weighted clocks 3,586 against 1,113. The thousandths truncation of `share` is not the cause (rescaled to 1e9: 1,900 against 943).
- Not yet isolated: the individual instructions whose depth changed. First step of the work: dump `(item, share, depth)` for both builds and diff them.

## (2) How often a verdict flips
Perturbation: the wide address trial on/off (the only layout-changing switch at hand; not layout-neutral, so these are upper bounds on pure-layout flips), QB 126 modules, -Os, module-level trial verdicts (hint, loop, inline, unroll):
- 68000: 2 of 126 modules flip (1 unroll put back -> kept, +466 B; 1 loop trial calls put back -> kept with 1 more refused inline site, +32 B). 68020: 0 of 126.
- Rare (~0.4% of module x CPU), but a flip is a whole-module decision (a size-for-clocks trade sized at 16x by `UNROLL_CLOCK_SHARE`). The per-function trial verdicts in `compile.rs` use the same `price_of` and were not counted.

## (3) The counterpart
- The pin: `MachineBlockFrequencyInfo` (branch probabilities + `MachineLoopInfo`, natural loops from dominators), never backward branches.
- We have it: `llrm-codegen/src/liveness/blockfrequency.rs` (`MachineBlockFrequencyInfo::of(body)`), used by the allocator, the coalescer, spill placement, `precse`, `slots`. The shared trial pricing used by the other targets (`backend/executed.rs` over `analysis::frequency`) is CFG-based too (branch heuristics, proven trip counts).
- m68k's `trial.rs` is the outlier: it prices a flat `Item` list with its own layout heuristic.

## Plan
1. Frequency source: compute block frequencies from the machine function through the existing `tolir` conversion + `MachineBlockFrequencyInfo` at the point `items_once` finishes, carry them to the emitted items by block label, and make `price_of` read them (weight = frequency relative to entry; depth no longer used). Calls keep the callee weighting of `price_of_functions`.
2. Class-level fail-first test (day 1, before the wiring): the same CFG laid out in two block orders must price identically; today `price_of` of two permutations of one loop nest differs. Plus a regression pin: the weighted clocks of a function are unchanged by inserting a forward jump that moves a block across a loop span.
3. Other 6 targets: untouched (m68k `trial.rs` only, behind an `Options` flag that is on after the A/B).
4. Compile cost: one MBFI per function build, already computed for the allocator on the same body (cache it): <1% expected.
5. A/B: all 126 modules x {68000, 68020} x {-Os, -O1}: module bytes (none larger or a dated row), verdict flips counted before and after, the 92 bench cells' instruction counts (clock model), md5 of the other targets.
6. Estimate: 3-4 days. Milestones: day 1 test + per-item diff naming the instructions; day 2 wiring; day 3 A/B; day 4 PR. Every m68k trial verdict can move, so the A/B is the gate.
7. Risk: verdicts that were right by luck on the old weights turn worse; the A/B shows them, each gets a row.
