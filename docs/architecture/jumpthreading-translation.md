---
architecture-index: JumpThreading as a translation of LLVM's
architecture-order: 276
---
# JumpThreading as a translation of LLVM's: the plan

TL;DR: `JumpThreading.cpp` (3,314 lines, pin `d1106deb71cc`) is translated into a new `jumpthreading.rs` on `LazyValueInfo` and `DomTreeUpdater`; the first slice (about 900 lines) is the driver, constant folding of terminators and `threadEdge` over a known-constant `phi`/LVI condition. Missing: `FindAvailableLoadedValue`, `SplitBlockPredecessors`, `cloneBasicBlock`'s value map, `TryToSimplifyUncondBranchFromEmptyBlock`, `DeleteDeadBlock`. Nothing here is built yet; every claim about the repo is from a grep at `33d100e5f`.

Rule for the code: each Rust function is a translation of the C++ function named in its comment (permalink at the pin); anything not translated is a numbered DEVIATE (J1, J2, ...) in the module header.

## 1. The functions

Base: [`JumpThreading.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp). Kind: MIRROR, DEVIATE (reason), SKIP (reason). Slice: S1 to S4 of section 4.

| function (line) | needs | kind | slice |
|---|---|---|---|
| [`run`/`runImpl` L240/L284](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L284) | `DomTreeUpdater` Lazy (`domtree_updater.rs`), LVI, `TryToSimplifyUncondBranchFromEmptyBlock` (T), `DeleteDeadBlock` (T), `RemoveRedundantDbgInstrs` | MIRROR; no TLI/TTI/AA arguments until S4 needs AA | S1 |
| [`findLoopHeaders` L527](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L527) | `FindFunctionBackedges` on the layout | MIRROR | S1 |
| [`processBlock` L948](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L948) | `ConstantFoldInstruction` (`constantfolding.rs`), `ConstantFoldTerminator` (T: private in `cvp.rs`), LVI `get_predicate_at`; the `indirectbr` arm has no MIR opcode (grep `opcode.rs`) | MIRROR minus `indirectbr` (DEVIATE J1) | S1 |
| [`computeValueKnownInPredecessorsImpl` L558](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L558) | LVI `get_constant_on_edge`, `simplify.rs`, ConstantRange | MIRROR | S1 |
| [`processThreadableEdges` L1572](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1572), [`evaluateOnPredecessorEdge` L1515](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1515) | the above, `tryThreadEdge`, `splitBlockPreds` (T) | MIRROR | S1 |
| [`tryThreadEdge` L2368](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2368), [`threadEdge` L2407](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2407), [`cloneInstructions` L2061](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2061), [`updateSSA` L1982](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1982) | `Edit::clone_instruction` + a value map (T: `ValueMapper` does not exist as a type; `looprotate.rs` keeps its own), `SsaUpdater` (`llrm-analysis/ssaupdater.rs`), LVI `thread_edge`, `codemetrics.rs` for the 6/3 size gate | MIRROR | S1 |
| [`maybeMergeBasicBlockIntoOnlyPred` L1919](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1919) | `merge_block_into_predecessor` (`cfg.rs:216`, `local.rs:754`; check which LLVM's `MergeBasicBlockIntoOnlyPred` the translation follows) | MIRROR | S1 |
| [`processBranchOnPHI L1753`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1753), [`processBranchOnXOR L1784`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1784) | `computeValueKnownInPredecessors` | MIRROR | S2 |
| [`tryToUnfoldSelect` L2886/L2924](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2886), [`tryToUnfoldSelectInCurrBB` L2982](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2982), [`unfoldSelectInstr` L2817](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2817) | LVI, block split | MIRROR | S2 |
| [`maybethreadThroughTwoBasicBlocks` L2151](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2151), [`threadThroughTwoBasicBlocks` L2292](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2292) | `cloneInstructions`, `updateSSA` | MIRROR | S2 |
| [`duplicateCondBranchOnPHIIntoPred` L2652](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2652), [`splitBlockPreds` L2497](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2497) | `SplitBlockPredecessors` (T: absent; `looprotate.rs`/`simpleloopunswitch.rs` have private splits, `domtree_updater.rs:275` only updates the trees) | MIRROR | S2 (split in S1) |
| [`processImpliedCondition` L1144](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1144) | `is_implied_condition` (`valuetracking/context.rs:1068`) | MIRROR | S3 |
| [`simplifyPartiallyRedundantLoad` L1222](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1222) | `FindAvailableLoadedValue` (T: no match in `crates`), AA (`basicaa.rs`), `SsaUpdater` | MIRROR once T exists | S4 |
| [`processGuards` L3119](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L3119), [`threadGuard` L3153](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L3153) | `llvm.experimental.guard` (an id exists: `valuetracking/pattern.rs:492`; the front end does not emit it, to check) | SKIP while no input has a guard (J2) | S4 |
| [`updateBlockFreqAndEdgeWeight` L2552](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2552), [`doesBlockHaveProfileData` L2541](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2541), BPI/BFI getters [L3275](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L3275) | `blockfrequency/` (BFI and branch probability, translated) | DEVIATE J3: `!prof` is not rewritten, BFI is not kept; LLVM does not preserve BPI/BFI either ([`getPreservedAnalysis` L3226](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L3226)) | S4 |

T = missing translation, a TODO row (section 3). Present and used as they are: LVI `get_value_on_edge`, `get_constant_on_edge`, `get_predicate_at`, `thread_edge`, `forget_value`, `erase_block` (`lazyvalueinfo.rs:2097`, `2694`, `2800`, `2968`, `2981`, `2991`).

## 2. Next to what runs today

`jumpthread.rs` (1,378 lines) is LLVM's `DFAJumpThreading`: a `switch` on a loop's state phi, copied per constant path. It is not `JumpThreading`; registry slot `dfa-jump-threading` is `Missing` in `parity.rs` and stays so until that file is renamed or translated. The translation gets its own `jumpthreading.rs`, and the old file is left alone.

`jump-threading` has no slot of its own: it is named in the `llvm` column of `decide@809` ("jump-threading, correlated-propagation", `Old`, `simplifycfg-port`). `decide.rs` threads a block over a condition a predecessor decides, and so overlaps S1/S2; [decide-vs-llvm.md](decide-vs-llvm.md) measures it (124.7 M on `d_faces` -O2 against 4.5 M for LLVM's JumpThreading). The translation adds two slots and splits `decide@809` into `jumpthreading@809` and the rest, so `check` keeps matching the pinned list.

## 3. Missing pieces (TODO rows for `parity.rs`)

| piece | LLVM | in the repo |
|---|---|---|
| `FindAvailableLoadedValue` | `Loads.cpp` | none (grep `available_loaded`/`find_available` finds only `SsaUpdater` internals) |
| `SplitBlockPredecessors`, `SplitEdge`, `SplitBlock` | `BasicBlockUtils.cpp` | done: `edges.rs` (`split_block_predecessors`, `split_edge`, `split_block`); the private copies in `simpleloopunswitch.rs` are gone |
| value-mapped block clone (`CloneBasicBlock`, `ValueMapper`) | `CloneFunction.cpp`, `ValueMapper.cpp` | done: `Function::clone_block` and `remap_instruction` with `ValueMap` (`edit.rs`); `looprotate.rs` still maps by hand |
| `ConstantFoldTerminator` | `Local.cpp` | open: a switch-only method of `Cvp` (`cvp.rs:2667`) on 14 `Cvp` helpers, among them its own `BasicBlock::removePredecessor`, of which `looprotate.rs`, `simpleloopunswitch.rs` and `loopdeletion.rs` hold three more private copies; the move starts with one shared `remove_predecessor` |
| `TryToSimplifyUncondBranchFromEmptyBlock`, `DeleteDeadBlock` | `Local.cpp`, `BasicBlockUtils.cpp` | `simplifycfg.rs:467` (`empty_block_removed`, not a translation); `loopdeletion.rs` erases its own |

## 4. Slots, preserved analyses, switch

The pinned list ([slots](parity-pipeline-slots.md)) runs `jump-threading` twice per function at -O2, -Os and -O3 (not -O1), after `early-cse<memssa>` and after `dfa-jump-threading`; 6 runs summed at x_hanoi2, 22 at d_faces, at -O2 and -O3 (run-count tables). Size gate: `jump-threading-threshold` 6, 3 for `minsize` ([L310](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L310)), 76 phis ([`PhiDuplicateThreshold`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L99)); no threading across loop headers.

Preserved: LVI and the dominator tree (`PreservedAnalyses` keeps both through `preserve`; the translation updates them through `DomTreeUpdater` and `LVI::thread_edge`), nothing else. Rule: ON by default under `llvm_order`, the replaced `decide` threading off behind `LLRM_NO_JUMPTHREADING` (presence, never `=0`); old rounds keep `decide` unchanged.

## 5. Stages and oracles

| stage | what | oracle | size |
|---|---|---|---|
| S1 | driver, `findLoopHeaders`, `processBlock` (fold terminator, `getKnownConstant`, LVI predicate), `processThreadableEdges`, `threadEdge`, `splitBlockPreds`, `cloneInstructions`, `updateSSA`, merge-into-only-pred, dead-block delete | `tools/jumpthreading_oracle.py` against `opt -passes=jump-threading` on the corpus, as `unrollfull_oracle.py` does: functions after cleanup identical, a control without the pass differs | about 900 lines |
| S2 | `processBranchOnPHI/XOR`, select unfolding, two-block threading, `duplicateCondBranchOnPHIIntoPred` | same | about 600 |
| S3 | `processImpliedCondition` | same | about 100 |
| S4 | partially redundant loads (needs `FindAvailableLoadedValue`), guards, profile (J2, J3) | same; guards and profile only where an input has them | about 600 |

A slice joins when its oracle shows 0 functions differing; a difference is classified as a numbered DEVIATE or fixed.

Fail-first tests for S1, one per shape, each seen failing with the shape's code stubbed: (1) a `br` on a `phi` of constants in the block is threaded from each constant predecessor; (2) a condition LVI proves on one edge is threaded from that edge only; (3) a block over the 6-instruction limit is not copied, 3 with `minsize`; (4) a loop header is not threaded across; (5) a copied value used below gets one `phi` from `SsaUpdater`; (6) `thread_edge` leaves LVI answering as a new LVI does; (7) an empty predecessor-less block is deleted.
