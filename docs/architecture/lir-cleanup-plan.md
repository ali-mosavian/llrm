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

All llrm figures are one instrument: callgrind inclusive of each phase's `transform_raising` on `llrm-c` built `--profile dist` from main `703d070b7` (`CARGO_INCREMENTAL=0`, no `RUSTFLAGS`), `d_faces.c` `-m32 -march=i486 -fno-inline-functions -O2` with the QCport includes, total 2,178.9 M. They differ from the coordinator's earlier pinned table (peephole 82.2, jumps 56.6, coalesce 42.8, x87 38.8 + 20.5) by -9% to +27%, which is a different build or tree, not a rule: the pinned ones are not used here. Shrink-wrap (11.2) is the only figure not re-taken (it is not a `transform_raising` phase).

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

## The peephole: what fires, and where each rewrite lives in LLVM

The peephole is 44 sub-passes run in a fixed order over each body (86.9 M of the 315 M). Counted with a counter on each (changed, instructions added, memory operands added, instructions rewritten in place; `LLRM_PEEPHOLE_STEPS`, a spike, not merged), over the corpus of `tools/identical.py` (803 compiles at `-m32`, 4,960 body runs per sub-pass) and the bench programs and `tests/run/c` at `-m16` with `i386`, `i486`, `pentium`, `core2` at `-O1/-O2/-Os` (1,608 compiles, 1,596 with output; the first m16 count used an `-march` the target does not know and lost 402 of 804, which the page that reported it did not use):

- 6 sub-passes never fire in either corpus (`frame`, `narrowed_arithmetic`, `restored_copies`, `secondary_bases`, `storecombine`, `waits`) and 5 more fire at most 12 times in 6,760 runs (`exact_addresses` 2, `pushed_constants` 2, `widened_moves` 2, `copyprop2` 10, `upperzero_hoisted` 8; `fused_calls` 6).
- What fires is mostly an in-place rewrite of one instruction to a cheaper form, not a removal: `zeroes` 55.9% of runs, `zero_compares` 39.9%, `increments` 38.1%, `fused` 33.9%, `tested` 27.2%, `copyprop` 23.1%, `extensions` 12.8%; the instruction count changes only in `addresses` (33.5%), `far_loads` (18.0%), `pushes`, `high_extracts`, `popped_arguments`, `sharedstores`, `zeroed_loads`; memory operands in `spillforward` (19.1%), `far_loads`, `addresses`, `machinedce`, `tested`.
- On `d_faces` `-O2` only 8.5% of sub-pass runs change the body and the net is -7 instructions in all. Cost beside fires (callgrind, release build of main `703d070b7` with the counter, not the dist build of the table above; ranking only): `copyprop` 13.2 M (fires 23%), `exact_addresses` 9.1 (fires 2 in 6,760), `spillforward` 7.8 (19%), `postrasink` 5.1 (14%), `machinecse` 4.3 (3.7%), `narrowed_arithmetic` 3.1 (never), `fused` 3.2 (34%), `regthrash` 3.0 (5.4%), `machinedce` 2.8 (2.4%), `zeroes` 2.6 (56%), the shared facts `allocate::live` 3.7, `dead_at_exit` 2.5, `_flags_live_out` 2.4.

Where the same rewrite lives in LLVM (every permalink read at `d1106deb71cc`; "not read" is a sub-pass whose home I have not yet found, and it stays until it is):

| llrm sub-pass | what it does | LLVM home | goes to |
|---|---|---|---|
| `zeroes` | `mov r, 0` to `xor r, r` | `MOV32r0` expanded after allocation by [`X86InstrInfo::expandPostRAPseudo`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L6411-L6416) | isel emits the pseudo once; post-RA work 0 |
| `zero_compares` | `cmp r, 0` to `test r, r` | isel patterns [`X86InstrCompiler.td`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrCompiler.td#L1493-L1500) | isel; 0 |
| `increments` | `add r, 1` to `inc r` | [`UseIncDec` patterns](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrCompiler.td#L819) | isel; 0 |
| `addresses`, `doubled`, `fused`, `fused_calls`, `far_loads` | address forms, folded loads | addressing-mode matching [`X86DAGToDAGISel::matchAddress`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L2053) and [`foldMemoryOperandImpl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L7773) | isel; 0 |
| `tested` | reuse the flags of the instruction before | [`X86InstrInfo::optimizeCompareInstr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L5473) (called by `PeepholeOptimizer`) | the translated `PeepholeOptimizer` |
| `narrowed_moves` | byte and word forms | [`X86FixupBWInstImpl::runOnMachineFunction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L165-L183) | the translated pass |
| `copyprop`, `copyprop2`, `regthrash` | forward and backward copy propagation | [`MachineCopyPropagation::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCopyPropagation.cpp#L1619-L1648) | the translated pass (the S5 block-local kill is re-tested there) |
| `postrasink`, `copysink` | sink a store or copy toward its use | [`PostRAMachineSinkingImpl::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp#L2375) | the translated pass |
| `machinecse` | redundant instruction elimination | [`MachineCSEImpl::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCSE.cpp#L937-L947) (before allocation in LLVM) | moves before allocation |
| `machinedce` | dead instruction elimination | [`DeadMachineInstructionElimImpl::runImpl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/DeadMachineInstructionElim.cpp#L79) | the translated pass |
| `spillforward` | a reload of a slot a register holds | [`InlineSpiller`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/InlineSpiller.cpp#L441) (hoisting and removing spills and reloads inside the allocator) | regalloc-core |
| `extensions`, `overwritten`, `pushes`, `popped_arguments`, `high_extracts`, `sharedstores`, `constants`, `transferred`, `frame_copies`, `zeroed_loads`, `compares_again`, `shuttles`, `upperzero_*`, `widened_moves`, `zero_extensions` | | not read | read before any is moved or deleted |

Ablation of the eleven that never or almost never fire (`LLRM_PEEPHOLE_SKIP`, rows `pl-pp-*`), worse rows first: removing all eleven worsens rows [clocks, executed instructions, memory operands, code bytes] of 71 at `-O1` [4, 3, 1, 2], `-O2` [3, 2, 1, 1], `-O3` [4, 3, 1, 3], `-Os` [1, 1, 0, 1]: `sieve` `-O1`/`-O2` clocks +20.9% (instructions +12.1%, memory operands +6.2%), `sieve` `-O3` clocks +7.1% (instructions +13.2%), `x_funcptr` instructions +7.7% and clocks +4.8% at every level, `quicksort` `-O1`, `queens` `-O3`, `shellsort`. By skipping one at a time: `copyprop2` (`sieve`, `quicksort`, `queens`), `upperzero_hoisted` (`sieve`, `shellsort`) and `fused_calls` (`x_funcptr`) are what those rows need. The other eight (`exact_addresses`, `secondary_bases`, `narrowed_arithmetic`, `restored_copies`, `waits`, `storecombine`, `pushed_constants`, `widened_moves`) change no row program: skipping them alone changes 7 of 1,583 objects (`run/idioms#3`, `#5`, `qcport/keybind -Os`, `r_portal -Omax`, `weapons -Os`, `sbar -O2`, `-Os`), 4 larger by 1, 1, 1 and 8 bytes, 2 smaller, net +3 bytes, and saves about 15 M (0.7% of the compile). They are not deleted now (a small gain for bytes on QCport objects): each gets its home and its rows check when it moves.

Jumps fixture (the translation of BranchFolding and MachineBlockPlacement must cover these rows with no worse clocks and keep the code win, or say which of the two LLVM's cost model gives; `preferred` is deleted in the PR that holds them and shows them not worse): without `preferred`, `x_rle` `-O1/-O2/-O3` clocks 43,680 to 43,683, instructions 18,836 to 18,837, code 266 to 262; `x_switch` `-O1` clocks +0.06%, instructions -0.10%, code 274 to 251; `x_switch` `-Os` clocks 120,827 to 121,397 (+0.47%), instructions +0.43%, code 228 to 199 (-12.7%). The cause: `preferred` chooses between the threaded layout without the merge and the layout after it by priced executed work, and without it the merge is always taken (LLVM's unconditional tail merge).

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
