---
architecture-index: JumpThreading as a translation of LLVM's
architecture-order: 276
---
# JumpThreading as a translation of LLVM's: the plan

TL;DR: `JumpThreading.cpp` (3,314 lines, pin `d1106deb71cc`) is translated into a new `jumpthreading.rs` on `LazyValueInfo` and `DomTreeUpdater`; the first slice (about 900 lines) is the driver, constant folding of terminators and `threadEdge` over a known-constant `phi`/LVI condition. Missing: `FindAvailableLoadedValue`, `SplitBlockPredecessors`, `cloneBasicBlock`'s value map, `TryToSimplifyUncondBranchFromEmptyBlock`, `DeleteDeadBlock`. S1 to S4 are built (`jumpthreading.rs`, `jumpthreading/loadpre.rs`, slots `jumpthreading@668` and `@809`, Partial); see section 6.

Rule for the code: each Rust function is a translation of the C++ function named in its comment (permalink at the pin); anything not translated is a numbered DEVIATE (J1, J2, ...) in the module header.

## Status: ON by default

On by default; `LLRM_NO_JUMPTHREADING` (set) turns it off. It was off from #2736 to this flip. The `datetime.c` -Os -m16 runaway was the pointer-chain walk in `decide` (fixed by #2739); JumpThreading only triggered it, and the unreachable cyclic group it leaves is what `opt` leaves too (guarded by #2744).

Downstream J12 (tracked against its owner, W2 decide / ipa-cp): `recpow.c` m16 -O2 compile costs +18.1% with it on (89.26 to 105.41 M instructions:u, A/A repeatable). The pass is about 1 M of the extra work; the bigger body comes from the old clocks inliner's `ipa-recursive.`/`ipa-peel.`. Exit: W2's `inlinecost::decide` lands.

Downstream J13 (tracked against its owner, W2 decide / ipa-cp): `queens` -O3 runs 96949 to 123141 clocks (+27%; instructions 78977 to 106279, memory ops 12911 to 25877). Our output equals `opt -passes=jump-threading`. A/B on the #2828 head (786569bd9), queens -O3 m32: forcing every ipa-cp candidate to clone (a throwaway switch skipping the verdict test, `ipacp.rs`) with it on gives 93729 clocks / 80393 instructions, below the off figure, so the clone decision is the lever. `LLRM_DEBUG=ipa-cp`: `_place` body size 24 (on) vs 26 (off); call-site frequency per clone level 9920, 36448, 337548, 3254456, 31512005 off (about x10 each) and 12124, 13976, 6377, 481, 8 on; `benefit*1000 >= EVAL_THRESHOLD(500)*size` fails at the sixth clone with it on (benefit 2, 12 needed), so `.6`-`.8` are never made. Our block frequencies equal `opt`'s `print<block-freq>` on the same IR, on and off (stage 11 and the stage-12 clones): the estimator is not at fault, the threaded bodies make the call sites cold. Owner: cycles-queue (ipa-cp clone verdict). Exit: W2's `inlinecost::decide` lands.

Downstream J14 (tracked against its owners, ShrinkWrap + MachineSink): `hanoi` -O2 runs 109 to 114 clocks. Our output equals `opt`. With it on `_hanoi` does `push ecx; mov ecx, eax` before the `n == 0` test, so the early return pays frame setup (shrink-wrap) and the constant copy sits before the test (pre-RA MachineSink). Exit: the ShrinkWrap port (backend-two) and the MachineSink translation (lir-cleanup-port).

Rows pair (one dist binary md5 80be384587ea, feat/parity 846c0c0c6, on against off, result `pjt-rows.txt`): clocks geomean llrm/min(gcc,clang) O2 .9149 to .9161 (+0.1%), O3 .8851 to .8889 (+0.4%), Os .9280 to .9333 (+0.6%); 13 rows worse, 16 better. Code bytes m16 -184/-286/-401 (O2/O3/Os), m32 -470/-890/-152.

Next: `decide`'s own threading is deleted (481f69460); S4: the load PRE is built (`jumpthreading/loadpre.rs`); guards (J2) and the profile (J3) stay deviations (section 6). The old `jumpthread.rs` is DFAJumpThreading-like, not replaced by this pass; `dfa-jump-threading` is a Missing row owned by parity-pipeline.

## 1. The functions

Base: [`JumpThreading.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp). Kind: MIRROR, DEVIATE (reason), SKIP (reason). Slice: S1 to S4 of section 4.

| function (line) | needs | kind | slice |
|---|---|---|---|
| [`run`/`runImpl` L240/L284](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L284) | `DomTreeUpdater` Lazy (`domtree_updater.rs`), LVI, `TryToSimplifyUncondBranchFromEmptyBlock` (T), `DeleteDeadBlock` (T), `RemoveRedundantDbgInstrs` | MIRROR; no TLI/TTI/AA arguments until S4 needs AA | S1 |
| [`findLoopHeaders` L527](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L527) | `FindFunctionBackedges` on the layout | MIRROR | S1 |
| [`processBlock` L948](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L948) | `ConstantFoldInstruction` (`constantfolding.rs`), `ConstantFoldTerminator` (`simplifycfg::terminator_folded`), LVI `get_predicate_at`; the `indirectbr` arm has no MIR opcode (grep `opcode.rs`) | MIRROR minus `indirectbr` (DEVIATE J1) | S1 |
| [`computeValueKnownInPredecessorsImpl` L558](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L558) | LVI `get_constant_on_edge`, `simplify.rs`, ConstantRange | MIRROR | S1 |
| [`processThreadableEdges` L1572](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1572), [`evaluateOnPredecessorEdge` L1515](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1515) | the above, `tryThreadEdge`, `splitBlockPreds` (T) | MIRROR | S1 |
| [`tryThreadEdge` L2368](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2368), [`threadEdge` L2407](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2407), [`cloneInstructions` L2061](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2061), [`updateSSA` L1982](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1982) | `Edit::clone_instruction` + a value map (T: `ValueMapper` does not exist as a type; `looprotate.rs` keeps its own), `SsaUpdater` (`llrm-analysis/ssaupdater.rs`), LVI `thread_edge`, `codemetrics.rs` for the 6/3 size gate | MIRROR | S1 |
| [`maybeMergeBasicBlockIntoOnlyPred` L1919](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1919) | `merge_block_into_predecessor` (`cfg.rs:216`, `local.rs:754`; check which LLVM's `MergeBasicBlockIntoOnlyPred` the translation follows) | MIRROR | S1 |
| [`processBranchOnPHI L1753`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1753), [`processBranchOnXOR L1784`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1784) | `computeValueKnownInPredecessors` | MIRROR | S2 |
| [`tryToUnfoldSelect` L2886/L2924](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2886), [`tryToUnfoldSelectInCurrBB` L2982](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2982), [`unfoldSelectInstr` L2817](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2817) | LVI, block split | MIRROR | S2 |
| [`maybethreadThroughTwoBasicBlocks` L2151](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2151), [`threadThroughTwoBasicBlocks` L2292](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2292) | `cloneInstructions`, `updateSSA` | MIRROR | S2 |
| [`duplicateCondBranchOnPHIIntoPred` L2652](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2652), [`splitBlockPreds` L2497](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2497) | `SplitBlockPredecessors` (T: absent; `looprotate.rs`/`simpleloopunswitch.rs` have private splits, `domtree_updater.rs:275` only updates the trees) | MIRROR | S2 (split in S1) |
| [`processImpliedCondition` L1144](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1144) | `is_implied_condition` (`valuetracking/context.rs:1068`) | MIRROR | S3 |
| [`simplifyPartiallyRedundantLoad` L1222](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1222) | `find_available_loaded_value` and `find_available_ptr_load_store` (`llrm_analysis::loads`), BasicAA through `BatchAAResults`, `is_safe_to_speculatively_execute_in` (`licm/hoist.rs`), `split_block_predecessors` | MIRROR (J15) | S4, done |
| [`processGuards` L3119](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L3119), [`threadGuard` L3153](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L3153) | `llvm.experimental.guard` (an id exists: `valuetracking/pattern.rs:492`; the front end does not emit it, to check) | SKIP (J2): a guard needs a `"deopt"` operand bundle, which `opt`'s verifier demands and MIR cannot hold; no front end emits one | S4 |
| [`updateBlockFreqAndEdgeWeight` L2552](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2552), [`doesBlockHaveProfileData` L2541](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2541), BPI/BFI getters [L3275](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L3275) | `blockfrequency/` (BFI and branch probability, translated) | DEVIATE J3: `!prof` is not rewritten, BFI is not kept; LLVM does not preserve BPI/BFI either ([`getPreservedAnalysis` L3226](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L3226)) | S4 |

T = missing translation, a TODO row (section 3). Present and used as they are: LVI `get_value_on_edge`, `get_constant_on_edge`, `get_predicate_at`, `thread_edge`, `forget_value`, `erase_block` (`lazyvalueinfo.rs:2097`, `2694`, `2800`, `2968`, `2981`, `2991`).

## 2. Next to what runs today

`jumpthread.rs` (1,378 lines) is LLVM's `DFAJumpThreading`: a `switch` on a loop's state phi, copied per constant path. It is not `JumpThreading`; registry slot `dfa-jump-threading` is `Missing` in `parity.rs` and stays so until that file is renamed or translated. The translation gets its own `jumpthreading.rs`, and the old file is left alone.

`jump-threading` has no slot of its own: it is named in the `llvm` column of `decide@809` ("jump-threading, correlated-propagation", `Old`, `simplifycfg-port`). `decide.rs` threads a block over a condition a predecessor decides, and so overlaps S1/S2; [decide-vs-llvm.md](decide-vs-llvm.md) measures it (124.7 M on `d_faces` -O2 against 4.5 M for LLVM's JumpThreading). The translation split `decide@809` into `jumpthreading@809` and the rest; the rest is deleted.

## 3. Missing pieces (TODO rows for `parity.rs`)

| piece | LLVM | in the repo |
|---|---|---|
| `FindAvailableLoadedValue` | `Loads.cpp` | none (grep `available_loaded`/`find_available` finds only `SsaUpdater` internals) |
| `SplitBlockPredecessors`, `SplitEdge`, `SplitBlock` | `BasicBlockUtils.cpp` | done: `edges.rs` (`split_block_predecessors`, `split_edge`, `split_block`); the private copies in `simpleloopunswitch.rs` are gone |
| value-mapped block clone (`CloneBasicBlock`, `ValueMapper`) | `CloneFunction.cpp`, `ValueMapper.cpp` | done: `Function::clone_block` and `remap_instruction` with `ValueMap` (`edit.rs`); `looprotate.rs` still maps by hand |
| `ConstantFoldTerminator` | `Local.cpp` | done: `simplifycfg::terminator_folded` (MIRROR, D4: no weights, no `DomTreeUpdater`); `cvp.rs` keeps its `switch` arm on `Cvp`'s LVI and weights. `BasicBlock::removePredecessor` and `removeIncomingValue` are one: `Function::predecessor_removed`/`remove_incoming_value` (`edit.rs`; swap-with-last as LLVM, which the old order-preserving removal was not); the private copies in `cvp.rs`, `looprotate.rs`, `simpleloopunswitch.rs`, `loopdeletion.rs` are gone |
| `TryToSimplifyUncondBranchFromEmptyBlock`, `DeleteDeadBlock` | `Local.cpp`, `BasicBlockUtils.cpp` | done: `simplifycfg::empty_block_removed` (MIRROR) and `dead_blocks_deleted` (`DeleteDeadBlocks`), both `pub`; `loopdeletion.rs` still erases its own |

## 4. Slots, preserved analyses, switch

The pinned list ([slots](parity-pipeline-slots.md)) runs `jump-threading` twice per function at -O2, -Os and -O3 (not -O1), after `early-cse<memssa>` and after `dfa-jump-threading`; 6 runs summed at x_hanoi2, 22 at d_faces, at -O2 and -O3 (run-count tables). Size gate: `jump-threading-threshold` 6, 3 for `minsize` ([L310](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L310)), 76 phis ([`PhiDuplicateThreshold`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L99)); no threading across loop headers.

Preserved: LVI and the dominator tree (`PreservedAnalyses` keeps both through `preserve`; the translation updates them through `DomTreeUpdater` and `LVI::thread_edge`), nothing else. Rule: the slots sit in `llvm_order`; the pass runs unless `LLRM_NO_JUMPTHREADING` is set (presence, never `=0`); `decide` no longer threads (481f69460).

## 5. Stages and oracles

| stage | what | oracle | size |
|---|---|---|---|
| S1 | driver, `findLoopHeaders`, `processBlock` (fold terminator, `getKnownConstant`, LVI predicate), `processThreadableEdges`, `threadEdge`, `splitBlockPreds`, `cloneInstructions`, `updateSSA`, merge-into-only-pred, dead-block delete | `tools/difftest/difftest.py` (`jumpthreading` against `opt -passes=jump-threading`, as for `cvp`): functions identical, a control where the reference never acts reads PREMISE FAILED | about 900 lines |
| S2 | `processBranchOnPHI/XOR`, select unfolding, two-block threading, `duplicateCondBranchOnPHIIntoPred` | same | about 600 |
| S3 | `processImpliedCondition` | same | about 100 |
| S4 | partially redundant loads (`loadpre.rs`, done), guards (J2) and profile (J3), both kept as deviations | same; eleven `load_pre_*` fixtures | about 300 (load PRE) |

A slice joins when its oracle shows 0 functions differing; a difference is classified as a numbered DEVIATE or fixed.

Fail-first tests for S1, one per shape, each seen failing with the shape's code stubbed: (1) a `br` on a `phi` of constants in the block is threaded from each constant predecessor; (2) a condition LVI proves on one edge is threaded from that edge only; (3) a block over the 6-instruction limit is not copied, 3 with `minsize`; (4) a loop header is not threaded across; (5) a copied value used below gets one `phi` from `SsaUpdater`; (6) `thread_edge` leaves LVI answering as a new LVI does; (7) an empty predecessor-less block is deleted.

## 6. State of S1 to S3

`jumpthreading.rs` ports S1 to S4; `decide@675` no longer threads, and `decide@809` is deleted (it cost 19.1 M on `d_faces` -O2 for 3 or 4 of 65 objects, +5 to +24 B). Switch: `LLRM_NO_JUMPTHREADING` (set: off; on by default, see Status). Fifteen fixtures in `tests/jumpthreading` read identical to `opt` through `difftest.py pair`; each function of S2 and S3 was seen failing a fixture with the function stubbed. Deviations J1 to J15 are in the module header.

The instrument is the slot oracle, not the `decide` inputs the first S1 count used: `difftest.py inputs jumpthreading@668|@809` over `llrm-cc -O2 --dump` of `d_faces` (`LLRM_DUMP_INPUTS=1`), 3 functions per slot. After S1 three differed (`668/03`, `809/02`, `809/03`); after S3 all changed functions are identical (2 of 2 at each slot).

The four `decide` inputs of the first count, rerun on the S2+S3 build: the numbering moved when the JT slots came in (`10/0046` and `11/0046` are now run 0047 of `decide@832`; there is no run 0067 in bodies 10 and 11). All five `decide` slots (`@1244 @675 @766 @809 @832`) of bodies 10 and 11 were run through `llrm-pass jumpthreading` against `opt -passes=jump-threading`: 9 of 10 identical, among them both 0047 runs. `11/0004` (now `decide@1244`) differed; the cause and fix are in the table below. With the fix it is identical; the other nine were not rerun.

Shapes of the four (the first three each a fixture that `opt` changes and S1 left alone):

| input | shape | stage | fixture |
|---|---|---|---|
| `668/03`, `809/03` | `PredBB` (conditional branch, several predecessors) then `BB` testing a value the edge into `PredBB` decides, through LVI or a `phi` of `PredBB`; `opt` copies `PredBB` for that edge and threads it through `BB` | S2 `maybethreadThroughTwoBasicBlocks` | `two_block`, `two_block_phi` |
| `668/03`, `809/03` | after that, `BB` is reached only through `PredBB`, whose one predecessor branches on the same condition | S3 `processImpliedCondition` (LVI answers "unknown" on both sides, so it is not an LVI gap) | `implied_condition` |
| `809/02` | a `br i1` on a `phi i1` of two compares, each predecessor ending in `br` | S2 `duplicateCondBranchOnPHIIntoPred` via `processBranchOnPHI` | `dup_cond_branch` |
| `11/0004` (`decide@1244`) | `opt` leaves `%t = phi i16 [ 2, %pred ]` in a block that threading reduced to one predecessor; the pass had the constant at the use | not a stage: `runImpl` walked a snapshot of the layout, `opt` the live list, so the `.thread` copy placed after `PredBB` was not visited before the next block merged into it and folded the phi. Fixed: the walk follows the live layout | `new_block_visited` |

The fixtures `select_unfold_cmp`, `select_unfold_switch`, `select_in_curr_bb` and `branch_on_xor` cover the rest of S2; no input of `d_faces` needs them.

Reduction: `llvm-reduce` (20) with an "opcode histogram differs the same way" test cut `668/03` to nothing smaller than the full function because every cut changes the histogram; the shapes above were read from the block diff of the two answers and checked by running `opt` and the pass on the hand-written fixtures.

## 7. S4

`simplifyPartiallyRedundantLoad` ([L1222](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1222)) is `jumpthreading/loadpre.rs`, called from `processBlock` on the load a branch or switch tests, or the left operand of a compare against a constant. It uses `find_available_loaded_value` in the block of the load and `find_available_ptr_load_store` once per predecessor (the pin has no predecessor-walking overload), then single predecessors up to `DefMaxInstsToScan` (6) instructions in all, the same cap as the pin, so the work per load is bounded and the pass stays linear. `isSafeToLoadUnconditionally` is not used at the pin: the pin asks `isSafeToSpeculativelyExecute`, which is `licm::hoist::is_safe_to_speculatively_execute_in`. Eleven fixtures `load_pre_*` read identical to `opt -passes=jump-threading` through `difftest.py pair`: partial, all available, aliasing store, volatile, address made in the block, phi address, split predecessors, critical edge, local availability, cast, switch. With the function stubbed to `return false` the four positive named tests and the fixture sweep fail; with the volatile check removed the volatile test and the sweep fail.

Kept deviations: J2 (guards: no operand bundles in MIR, so no well-formed guard; `opt` rejects one without `"deopt"`) and J3 (`!prof`: the weights the pin rewrites after threading a block with valid branch weights; `difftest.py` ignores metadata). Not measured: the `d_faces` -O2 compile cost and assembly with S4.

`jumpthreading@668` and `@809` are Translated. Criterion: oracle identical on the d_faces -O2 inputs (`jumpthreading@668`: 11 compared, 3 changed, all identical; `@809`: 11 compared, 5 changed, all identical; `difftest.py` inputs on origin/feat/parity af6070a7a). Pass-internal DEVIATEs are J2 (guards), J3 (`!prof`) and J15 (`isSafeToSpeculativelyExecute` reuse), each with its exit in the module header. J12 to J14 are downstream effects, tracked against their owners (W2 `decide` / ipa-cp, ShrinkWrap + MachineSink), not exit conditions of this pass.
