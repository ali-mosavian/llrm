# LIR clean-up: each pass against LLVM's machine pass

TL;DR: on `d_faces` `-O2` the LIR passes after selection cost 315.5 M against about 49.6 M for clang's machine passes, a gap of 266 M, and until now had no plan. This page maps every LIR sub-pass to the LLVM pass it stands in for, with cost against cost, runs, owner and what deletes it. Four rows are 233 M of the gap: `peephole` (86.9 M against 12.6 M), `jumps` (51.6 against 5.9), the x87 pair (60.6 against 1.6) and `coalesce` (54.5 against 10.1, held for the allocator). Measured: callgrind inclusive, `d_faces` `-O2`, 11 functions, 2,250 LIR instructions (1,594 machine instructions in LLVM's six); llrm figures are the pinned ones where the table has them.

## The map

| llrm pass | llrm M | LLVM counterpart | LLVM M | runs (llrm / LLVM per function) | owner | deleted when | DEVIATEs (named extra work) |
|---|--:|---|--:|---|---|---|---|
| `peephole` (generated rule groups, 47 sub-passes) | 86.9 | X86 peephole 1.0 + MachineCSE 2.4 + MachineCopyPropagation 2.8 + DeadMachineInstructionElim 1.3 + MachineSink 4.0 + X86FixupBWInsts 1.1 | 12.6 | 1 / 1 each | lir-cleanup-poc | replaced by one forward walk per block with a live-register-unit set (`PeepholeOptimizer.cpp`, `MachineCopyPropagation.cpp`) | a fresh `Facts` and `_dead_after` map per rule group (20), whole-function liveness asked 8 times, fixed points up to 8 rounds (`regthrash`), 4 (`machinecse`), a gap walk per instruction ([lir-cleanup.md](../comparison/llvm/lir-cleanup.md)) |
| `jumps` | 51.6 | BranchFolding 2.0 + MachineBlockPlacement 3.9 (+ TailDuplicator) | 5.9 | 1 / 1 (+1 inside placement) | lir-cleanup-poc | the extras move to their LLVM homes as translations (below); the one with no home is decided by its rows | merge-and-DCE fixed point with a second threading (23.0): home `BranchFolder::OptimizeFunction`/`TailMergeBlocks`/`OptimizeBranches`; tail and return duplication (8.4): home `TailDuplicator::tailDuplicateBlocks` and `MachineBlockPlacement::maybeTailDuplicateBlock` (limit 2 instructions, 4 aggressive); rotation (6.6): home `MachineBlockPlacement::rotateLoop`; baseline-vs-merged pricing choice (9.7): NO LLVM home. ~47.7 M of the 57.9 M; placement itself ~10 M |
| `floatassign` + `floatalloc` | 39.6 + 21.0 | X86FloatingPoint (FPStackifier) | 1.6 | 1 / 1 on 4 of 11 functions | lir-cleanup-poc with regalloc-core | MIRROR, not deviate: LLVM's own design is seven flat x87 registers the allocator colours and a stackifier that is one forward walk per block (`FPS::run`); our pair is deleted when the allocator has the class | a whole IRA/LRA-style allocator for the stack (allocnos, spill placement, Belady victim rounds, rebuilt body per round), two liveness solves, `_modes` fixed points |
| `coalesce` | 54.5 | RegisterCoalescer 7.0 + PHIElimination 2.1 + TwoAddress 1.0 | 10.1 | 1 / 1 | regalloc-core-poc | held: re-measured on the allocator core ([lir-jumps-coalesce.md](../comparison/llvm/lir-jumps-coalesce.md), #2110) | whole-function interference graph + Briggs/George (29.7 M before the first copy); joins by interval overlap in #2110 |
| `phielim` | 9.9 | PHIElimination | 2.1 | 1 / 1 | regalloc-core-poc | with the allocator's `LiveIntervals` | edge prices, copies placed by cost |
| `shrink-wrap` / `prologue` | 11.2 | ShrinkWrap + PrologEpilogInserter | not measured | 1 / 1 | lir-cleanup-poc | measure LLVM's first | not read yet |
| `parcopy` | 8.7 | (parallel copies resolved by the register rewriter and `TwoAddress`) | n/a | 1 / n/a | regalloc-core-poc | with the allocator's parallel-copy handling | cycle breaking priced per CPU |
| `twoaddr` | 7.3 | TwoAddressInstructionPass | 1.0 | 1 / 1 | #2079 (pair queries), lir-cleanup-poc | #2079 lands: the graph is not built | builds a partial interference graph for commute candidates |
| `pressuresink` | 7.9 | MachineSink (counted in the peephole row) | n/a | 1 / 1 | lir-cleanup-poc | folded into the `peephole` replacement | a load made where its reader is, before any value is placed |
| `loopslots` | 6.1 | StackSlotColoring | 0.8 | 1 / 1 | lir-cleanup-poc | measure why 9x | which slots a loop still reaches |
| `phiconst` / `floatfold` / `farcall` / `scheduler` | 1.7 / 0.8 / 0.1 / 0.0 | (MachineScheduler 9.2, no counterpart) | 9.2 | 1 / 1 | n/a | n/a | `scheduler` is nearly free here because it moves nothing; LLVM's 9.2 M buys code quality this table does not price |

All llrm figures are one instrument: callgrind inclusive of each phase's `transform_raising` on `llrm-c` built `--profile dist` from main `703d070b7` (`CARGO_INCREMENTAL=0`, no `RUSTFLAGS`), `d_faces.c` `-m32 -march=i486 -fno-inline-functions -O2` with the QCport includes, total 2,178.9 M. They differ from the coordinator's earlier pinned table (peephole 82.2, jumps 56.6, coalesce 42.8, x87 38.8 + 20.5) by -5% to +27%, which is a different build or tree, not a rule: the pinned ones are not used here. Shrink-wrap (11.2) is the only figure not re-taken (it is not a `transform_raising` phase).

LLVM figures: `llc -O2 -mtriple=i486-pc-linux-gnu` on the optimized IR of `d_faces.c`, callgrind inclusive of each pass's `runOnMachineFunction`, 216.6 M in all; the six functions, not the eleven ([#1994 body-size table](https://github.com/ali-mosavian/llrm/issues/1994)). The `llc` build used there is body-size-table's and its version string was not recorded in that comment; the C++ read for the translations is d1106deb71cc, and the local `llc-20` is Ubuntu LLVM 20.1.8, an optimized build (re-measure with it before a translation PR quotes a ratio). The two columns are by purpose, not by identical work.

## The C++ passes

Each row's counterpart, at `d1106deb71cc`:

- [`X86FloatingPoint (FPS::run)`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FloatingPoint.cpp#L347-L394)
- [`PeepholeOptimizer::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PeepholeOptimizer.cpp#L1776-L1976)
- [`MachineCSEImpl::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCSE.cpp#L937-L947)
- [`MachineCopyPropagation::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCopyPropagation.cpp#L1619-L1648)
- [`MachineSinking::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp#L840-L957)
- [`X86FixupBWInstImpl::runOnMachineFunction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L165-L183)
- [`BranchFolder::OptimizeFunction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L237-L316)
- [`BranchFolder::TailMergeBlocks`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L1109-L1257)
- [`BranchFolder::OptimizeBranches`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L1307-L1329)
- [`MachineBlockPlacement::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockPlacement.cpp#L3568-L3678)
- [`MachineBlockPlacement::rotateLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockPlacement.cpp#L2423-L2496)
- [`MachineBlockPlacement::maybeTailDuplicateBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockPlacement.cpp#L3213-L3318)
- [`TailDuplicator::tailDuplicateBlocks`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp#L274-L299)
- [`ShrinkWrapImpl::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/ShrinkWrap.cpp#L945-L996)
- [`PHIEliminationImpl::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PHIElimination.cpp#L236-L315)
- [`TwoAddressInstructionImpl::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L1879-L2023)
- [`StackSlotColoring::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/StackSlotColoring.cpp#L526-L564)
- [`PEIImpl::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PrologEpilogInserter.cpp#L211-L349)
- [`DeadMachineInstructionElimImpl::runImpl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/DeadMachineInstructionElim.cpp#L79-L91)

## Order

1. **`peephole` and `pressuresink`** (90 M against 12.6): the largest row and the cheapest to attack, because no other session owns it. Replace the 47 sub-passes by the LLVM set in the order `PeepholeOptimizer`, `MachineCSE`, `MachineCopyPropagation`, `MachineSink`, translated from the C++ with the file open (L-numbers in the PR); shadow mode first, edits missed classified, then the cost per instruction against 7.9 K.
2. **The x87 pair** (59.3 against 1.6, 37x): the biggest ratio. Depends on the allocator colouring seven flat x87 registers; the stackifier then is `FPS::run` one forward walk. Plan with regalloc-core.
3. **`jumps`**: classify first, then measure. Merge, duplication and rotation have LLVM homes (above): each becomes a translation of that function with the C++ open, and the ablation rows only say what the translation must reproduce. The pricing choice (`preferred`) has no LLVM home: the ablation rows (`LLRM_JUMPS_NO_PREFER`) decide it; deleted unless a row needs it, then a DEVIATE with its row.
4. **`coalesce`, `phielim`, `parcopy`**: with regalloc-core (S1/S2).
5. **`shrink-wrap`, `loopslots`**: measure LLVM's number first.

## Re-test items

- **S5 block-local copyprop** (measured as a kill in today's structure): re-test inside the new `MachineCopyPropagation` translation, where the live-register-unit set is the structure it needs, not the whole-function liveness it was tried against.
- The interval-join coalescer's per-row worse counts (#2110) on the new allocator.
- `-fexcess-precision=standard` float rows (#2126) once the rows tool is wired to `tools/precision.py`.

## Not known

- LLVM's ShrinkWrap and PrologEpilogInserter cost in the same `llc` run.
- The per-instruction cost of each llrm pass over its own input size (the table divides by the allocator's input).
- Whether the 11-function llrm compile and the 6-function llc one weight the passes alike (LLVM inlines the statics).
