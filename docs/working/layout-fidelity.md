# Block layout of new blocks

A pass that visits blocks in layout order (JumpThreading, SimplifyCFG, GVN) and the register allocator read where a helper put a new block. The pin places it by helper; ours appended at the end. x_list's sum loop kept two pointer phis for it (with `canonicalizeICmpPredicate`, below).

| helper | pin | ours |
|---|---|---|
| `SplitBlockPredecessors` | right before BB (BasicBlockUtils.cpp:1331) | before BB (this change) |
| `SplitKnownCriticalEdge` | right after the source (BreakCriticalEdges.cpp:186) | after the source (this change) |
| LoopSimplify preheader | `placeSplitBlockCarefully` (LoopSimplify.cpp:80) | the same |
| LoopSimplify exit, backedge | before the exit; after the last latch | the same |
| `splitBasicBlock`, `SplitEdge` | after the old block; three cases (BasicBlockUtils.cpp:672) | the same (#4560) |
| LoopUnroll copies | in order right after the latch (LoopUnroll.cpp:1214) | the same (#4572) |
| LoopRotate `ExitSplit` | before the exit (LoopRotationUtils.cpp:788) | the same (#4572) |
| uncondicmp `switch.edge` | before the compare's block (SimplifyCFG.cpp:5463) | the same (#4572) |
| `breakLoopBackedge` | `SplitEdge(Latch, Header)` (LoopUtils.cpp:800) | the same (#4572) |
| LSR split of a loop exit | after the source, moved before the phi's block when the source is in the loop and the phi is not (LoopStrengthReduce.cpp:5998) | the same |
| `peelLoop` | `peel.begin`, `peel.next`, `peel.newph` around the clones (LoopPeel.cpp:1296) | open: `loopclone::peeled_any_latch` makes no such blocks, so its CFG differs from the pin's; no measured effect (d_faces -O2 unrolls 1 of 11 functions), not rewritten blind |

Measured against b967bfa0e (release, d_faces -O2): 1,015,182,862 -> 1,012,850,850 Ir. Objects move in 6 to 11 sources per level; CODE bytes m16 -O2 +408, m32 -O2 +0.

## Cells above the base

Each is the same IR laid out as the pin lays it out; the register allocator then picks other registers. quicksort m16 -O2 414 -> 423: the frame is `sub sp, 12` for `sub sp, 10`, one more spill.

| cell | bytes |
|---|---|
| m16 -O2 huge_runs, jpeg_single, jpeg_fixed, huge_fill, quicksort | 8610->8755, 7295->7418, 8218->8333, 418->434, 414->423 |
| m16 -O3 huge_runs, huge_fill, quicksort, idioms | 8948->9080, 418->434, 500->509, 2454->2459 |
| m16 -Os quicksort, jpeg_fixed | 249->258, 2906->2911 |
| m32 -O1 x_rle, x_list | 267->299, 187->203 |
| m32 -O2 jpeg_single, grep | 10161->10193, 697->713 |
| m32 -O3 jpeg_fixed, idioms, grep | 10247->10279, 3882->3913, 713->729 |
| m32 -Os memfuncs_runtime, x_rle, grep, jpeg_fixed | 392->406, 198->206, 546->551, 3376->3381 |
