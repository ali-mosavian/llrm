---
architecture-index: ShrinkWrap as a translation of LLVM's
architecture-order: 276
---
# ShrinkWrap as a translation of LLVM's: the plan

TL;DR: `ShrinkWrap.cpp` and `PrologEpilogInserter`'s save/restore placement are translated into one backend module that picks one Save and one Restore block from the dominator and post-dominator trees, costed from the target description. It replaces `backend/shrinkwrap.rs`, which is gcc's per-piece scheme (`try_shrink_wrapping_separate`), not LLVM's. Row B7; fail-first is hanoi -Os (188 → 298 clocks: `push ecx` before the `n == 0` test).

Rule for the code: each Rust function names the C++ function it translates (permalink at `d1106deb71cc`); anything not translated is a numbered DEVIATE in the module header. No machine fact in the pass: save/restore costs and the frame cell size come from the target description (`TargetFrameLowering`/`TTI` analogue), which backend-two's audit places.

## Stages

| stage | what | check |
|---|---|---|
| S0 | why the current pass leaves hanoi's `push ecx` at the entry (its `WORTH` of three pieces, the join block `pop ecx; ret`, or the frame not being a piece), with the pass's decision dumped | the hanoi dump from a scratch directory |
| S1 | `ShrinkWrapImpl::run`: `useOrDefCSROrFI`, the Save walk over `MDT`, the Restore walk over `MPDT`, `isSaveReachableThroughClobber`, the loop and frequency rejection (`BlockFrequencyInfo` costs), `tryToReallocateCSR` | unit tests on LIR bodies with the pass's own expected Save/Restore; oracle against `llc` on the pin where a target exists |
| S2 | PrologEpilogInserter: `calculateSaveRestoreBlocks`, the frame cell and CSR saves at Save, the restores before each return Restore post-dominates; the stack depth stays one per block | hanoi final IR has no `push ecx` before the test; a return reached around Save is rejected |
| S3 | delete `backend/shrinkwrap.rs` once every row it won is held (rows pair by bench, not by total) | ratchet on the rows; `x_hanoi2`, main |

Cost: the dominator and post-dominator trees are built once per body (O(n)); each walk is linear in blocks. Slope test at 1× and 4× blocks.
