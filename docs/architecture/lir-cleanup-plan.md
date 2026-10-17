# LIR clean-up: each pass against LLVM's machine pass

TL;DR: on `d_faces` `-O2` the LIR passes after selection cost 315.5 M against about 49.6 M for clang's machine passes, a gap of 266 M, and until now had no plan. This page maps every LIR sub-pass to the LLVM pass it stands in for, with cost against cost, runs, owner and what deletes it. Four rows are 233 M of the gap: `peephole` (82.2 M against 12.6 M), `jumps` (56.6 against 5.9), the x87 pair (59.3 against 1.6) and `coalesce` (42.8 against 10.1, held for the allocator). Measured: callgrind inclusive, `d_faces` `-O2`, 11 functions, 2,250 LIR instructions (1,594 machine instructions in LLVM's six); llrm figures are the pinned ones where the table has them.

## The map

| llrm pass | llrm M | LLVM counterpart | LLVM M | runs (llrm / LLVM per function) | owner | deleted when | DEVIATEs (named extra work) |
|---|--:|---|--:|---|---|---|---|
| `peephole` (generated rule groups, 47 sub-passes) | 82.2 | X86 peephole 1.0 + MachineCSE 2.4 + MachineCopyPropagation 2.8 + DeadMachineInstructionElim 1.3 + MachineSink 4.0 + X86FixupBWInsts 1.1 | 12.6 | 1 / 1 each | lir-cleanup-poc | replaced by one forward walk per block with a live-register-unit set (`PeepholeOptimizer.cpp`, `MachineCopyPropagation.cpp`) | a fresh `Facts` and `_dead_after` map per rule group (20), whole-function liveness asked 8 times, fixed points up to 8 rounds (`regthrash`), 4 (`machinecse`), a gap walk per instruction ([lir-cleanup.md](../comparison/llvm/lir-cleanup.md)) |
| `jumps` | 56.6 | BranchFolding 2.0 + MachineBlockPlacement 3.9 | 5.9 | 1 / 1 (+1 inside placement) | lir-cleanup-poc | each LLVM-absent extra is deleted unless a row needs it (see below) | merge-and-DCE fixed point with a second threading (23.0), baseline-vs-merged pricing choice (9.7), tail and return duplication (8.4), rotation (6.6): ~47.7 M of the 57.9 M; placement itself ~10 M |
| `floatassign` + `floatalloc` | 38.8 + 20.5 | X86FloatingPoint (FPStackifier) | 1.6 | 1 / 1 on 4 of 11 functions | lir-cleanup-poc with regalloc-core | the x87 values are a register class of seven the allocator colours, and the stackifier is one forward walk per block | a whole IRA/LRA-style allocator for the stack (allocnos, spill placement, Belady victim rounds, rebuilt body per round), two liveness solves, `_modes` fixed points |
| `coalesce` | 42.8 | RegisterCoalescer 7.0 + PHIElimination 2.1 + TwoAddress 1.0 | 10.1 | 1 / 1 | regalloc-core-poc | held: re-measured on the allocator core ([lir-jumps-coalesce.md](../comparison/llvm/lir-jumps-coalesce.md), #2110) | whole-function interference graph + Briggs/George (29.7 M before the first copy); joins by interval overlap in #2110 |
| `phielim` | 12.1† | PHIElimination | 2.1 | 1 / 1 | regalloc-core-poc | with the allocator's `LiveIntervals` | edge prices, copies placed by cost |
| `shrink-wrap` / `prologue` | 11.2 | ShrinkWrap + PrologEpilogInserter | not measured | 1 / 1 | lir-cleanup-poc | measure LLVM's first | not read yet |
| `parcopy` | 9.5† | (parallel copies resolved by the register rewriter and `TwoAddress`) | n/a | 1 / n/a | regalloc-core-poc | with the allocator's parallel-copy handling | cycle breaking priced per CPU |
| `twoaddr` | 8.0† | TwoAddressInstructionPass | 1.0 | 1 / 1 | #2079 (pair queries), lir-cleanup-poc | #2079 lands: the graph is not built | builds a partial interference graph for commute candidates |
| `pressuresink` | 7.8† | MachineSink (counted in the peephole row) | n/a | 1 / 1 | lir-cleanup-poc | folded into the `peephole` replacement | a load made where its reader is, before any value is placed |
| `loopslots` | 7.2† | StackSlotColoring | 0.8 | 1 / 1 | lir-cleanup-poc | measure why 9x | which slots a loop still reaches |
| `phiconst` / `floatfold` / `farcall` / `scheduler` | 1.9† / 0.9† / 0.2† / 0.0† | (MachineScheduler 9.2, no counterpart) | 9.2 | 1 / 1 | n/a | n/a | `scheduler` is nearly free here because it moves nothing; LLVM's 9.2 M buys code quality this table does not price |

† Not a pinned figure: my callgrind inclusive on a binary of main from 2026-10-17 (before the pinned run). The pinned ones (peephole, jumps, coalesce, the x87 pair, shrink-wrap) are the coordinator's table.

LLVM figures: `llc -O2 -mtriple=i486-pc-linux-gnu` on the optimized IR of `d_faces.c`, callgrind inclusive of each pass's `runOnMachineFunction`, 216.6 M in all; the six functions, not the eleven ([#1994 body-size table](https://github.com/ali-mosavian/llrm/issues/1994)). The `llc` build used there is body-size-table's and its version string was not recorded in that comment; the C++ read for the translations is d1106deb71cc, and the local `llc-20` is Ubuntu LLVM 20.1.8, an optimized build (re-measure with it before a translation PR quotes a ratio). The two columns are by purpose, not by identical work.

## Order

1. **`peephole` and `pressuresink`** (90 M against 12.6): the largest row and the cheapest to attack, because no other session owns it. Replace the 47 sub-passes by the LLVM set in the order `PeepholeOptimizer`, `MachineCSE`, `MachineCopyPropagation`, `MachineSink`, translated from the C++ with the file open (L-numbers in the PR); shadow mode first, edits missed classified, then the cost per instruction against 7.9 K.
2. **The x87 pair** (59.3 against 1.6, 37x): the biggest ratio. Depends on the allocator colouring seven flat x87 registers; the stackifier then is `FPS::run` one forward walk. Plan with regalloc-core.
3. **`jumps`**: decide each extra by the per-row worse counts of the ablation (rows running: `LLRM_JUMPS_NO_MERGE/PREFER/DUP/ROTATE`); an extra no row needs is deleted, one a row needs is named a DEVIATE with its row.
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
