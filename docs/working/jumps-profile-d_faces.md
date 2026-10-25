# ControlFlow (`jumps`) on d_faces -O2, m16 dag

2026-10-25, parity 3c36dac99, callgrind with line tables, `-fno-inline-functions`: 14.57 M of 820 M Ir (1.8%) over 3 bodies. The pin's whole `BranchFolder` on d_faces is 1.7 M (compile-time-review.md:1038, 2 runs, 965 block visits); `MachineBlockPlacement` is not in that figure.

| Part (inclusive Ir, calls) | Ir | Pin counterpart |
|---|---|---|
| `tailmerge::merged` (5) | 4.43 M | `TailMergeBlocks`, ~0.2 M a call |
| of it `loops::<Block>` dominance forest (2) | 1.03 M | `MachineLoopInfo`, computed once per function |
| of it `Folder::predecessors` (11) | 0.69 M | pred lists live in the CFG |
| of it `seq_of` (567) | 0.67 M | compares from the tail with iterators, no `Vec` |
| `threaded` -> `branchfold::folded` (5) | 3.43 M | `OptimizeBranches`, one visit a block |
| of it `Folder::rules` (1622 visits) | 2.16 M, 1.3 k Ir a visit | `OptimizeBlock` |
| `_placed` -> `placement::order` (3) | 2.20 M | MBP, MBFI shared with the rest |
| `Frequency::of` in `inverted` (2) | 1.96 M | MBFI kept up to date by `MBFIWrapper` |
| `machinedce::eliminated` (2) | 1.61 M | none (T6: LLVM leaves the compare) |

Reading:
- Both `Frequency::of` runs are `inverted` (jumps.rs:172), on the post-merge shape the carried table no longer fits. `aligned` runs none: reusing `inverted`'s frequency for it was tried and changes nothing.
- 3 of 5 `merged` calls are the confirming round that finds nothing; the pin pays the same rounds at about a fifth of the cost.
- `rules` is 0.3 M of its own lines; the rest is inlined `Arc`/`Option<Semantics>` work outside `branchfold.rs`.

Checked and not waste:
- `Frequency::of` in `inverted`: cached by CFG shape (`Worked`, `Carried`); the two runs follow a merge that changed the shape. The carried table costs more per run (3.5 M over its 2 runs) than a fresh run (0.5 M).
- The tail merge's loop forest: `analysis::loops::loops` is memoized by shape (`shaped`); the 2 runs in `merged` are misses after a real CFG change, and it is asked only once two predecessors hash alike.

Candidate cuts, none made: keep the predecessor map across rounds (~0.5 M); `seq_of` from the tail (~0.5 M). About 1 M, 0.1% of the compile.
