# The second frame: lay the frame out again, not run the backend again

TL;DR: for a function whose frame has operands past `[bp-128]`, `machined_once` runs the whole backend a second time with a hole above the allocas for spill slots, and keeps the run with fewer far operands. The second body equals the first with its frame displacements changed in 270 of 270 runs on QCport (m16, `-O0`, `-O2`, `-Os`), and the run costs 24.5% (`-O0`) to 35.8% (`-O2`) of the backend's time there. Proposal: compute the second layout from the first run's frame and body, keep the old second run as the reference under `LLRM_CHECK_FRAME`.

## What happens today

`assemble.rs` `machined_once`: run 1 with `hole = 0`; if `spilled = frame.floor + reserve > 0`, the frame has a floor and `far_frame(first) != 0`, run 2 with `hole = spilled`; keep run 2 if `far_frame(second) < far_frame(first)`. Each run is `cheaper`: the spiller candidate, and where it changed something the allocator-alone candidate. isel starts its stack depth at `hole` (`isel.rs` 733, 869-870, 4120-4122), so every cell isel makes (allocas, temporaries) lies `hole` bytes lower; `Frame::slot` (`frame.rs` 123-135) hands spill slots out of `[-hole, 0)` first, top down, and below the lowest home after that.

## Measured

Instrumented `machined_once` to compare the two bodies instruction by instruction with every BP-relative displacement zeroed (`Mem.addr.disp`, `offset`, `disp_width`, for frame cells and for SS-segment cells indexed through a register): QCport's 65 modules, 270 functions with a second run: identical in all 270 (56 at `-O0`, 108 at `-O2`, 106 at `-Os`). A first version of the comparison that skipped indexed cells reported 68% and was wrong; the 32% it called different were array cells whose displacement moved with the allocas.

Share of the backend (`assemble`) spent in the second run: QCport `-O0` 24.5%, `-O2` 35.8% (16 and 26 of 65 modules have one). The m32 bench kernels have almost none (their frames are small).

## Proposal

After run 1, build run 2's frame by replaying the slot hand-out: a `Frame::new(floor - hole)` with `hole`, asking `slot(key, capacity)` for each of run 1's slots in creation order; then map every frame cell of run 1's body: inside a slot `[home, home + capacity)` to the replayed home plus the offset; any other negative displacement at or above run 1's floor (an alloca or temporary) to `disp - hole`; positive (arguments) unchanged. `Machined.reserve`, `frame.floor`, and anything else derived from the layout follow. Choose by `far_frame` as now.

## What can differ, and the guard

A pass after allocation that decides by a displacement (the candidate `cost`, which prices encodings; peephole) could decide differently in a real second run. The measurement says it did not in 270 cases; it is not a proof. So: `LLRM_CHECK_FRAME=1` runs the real second run beside the computed one and asserts the two bodies and frames equal (displacements included); the gate runs it over the libs, bench, QCport at `-O0/-O2/-Os` and `-g`. Debug info that records a displacement from isel's side tables (not from the body) is the open point: `-g` builds are compared too, and the computed layout is not used where the check finds a difference in any program.

## Not done

No predictor of which run wins: the rule stays `far_frame`. The two candidates inside a run (spiller / allocator alone) are a separate question (a near coin flip per function, measured separately).

Expected: the second run's 24-36% of the backend on programs that have one; objects identical by construction of the check.
