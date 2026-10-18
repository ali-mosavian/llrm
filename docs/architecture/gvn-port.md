# GVN and MemoryDependenceAnalysis as a translation of LLVM's: the plan

TL;DR: `GVN.cpp`, `MemoryDependenceAnalysis.cpp` and `VNCoercion.cpp` (6,410 lines at `d1106deb71cc`) are translated function for function, in six small stages that each delete the llrm code they replace. Engine and options are those of `-O2`/`-O3`/`-Os` in the pinned `buildFunctionSimplificationPipeline`: MemoryDependenceAnalysis, scalar PRE, load PRE and load-in-loop PRE on, backedge-split load PRE off. No `-O1` change (see Out of scope).

Pinned: LLVM [`d1106deb71cc`](https://github.com/llvm/llvm-project/tree/d1106deb71cc81016406a1917d0508d2df296266). Each Rust function is a translation of the C++ function named in its first comment (permalink), in the same order of checks; anything not translated is a numbered DEVIATE with a stated fact and an assert or test at the boundary.

## 1. Configuration translated

| Option | Value | Source |
|---|---|---|
| `enable-gvn-memdep` | true | [GVN.cpp#L119](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L119) |
| `enable-gvn-memoryssa` | false: the MemorySSA engine (`findReachingValuesForLoad` and its helpers, L2330-L2790) is not translated | [GVN.cpp#L120](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L120) |
| scalar PRE, load PRE, load-in-loop PRE | on | [GVN.cpp#L111](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L111) |
| split-backedge load PRE | off | [GVN.cpp#L116](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L116) |
| `gvn-scan-users-limit` 100, `gvn-max-num-deps` 100, `gvn-max-num-reaching-blocks` 200, `gvn-max-block-speculations` 600, `gvn-max-num-visited-insts` 100, `gvn-max-num-insns` 100 | defaults | [GVN.cpp#L123-L153](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L123) |
| `memdep-block-scan-limit` 100, `memdep-block-number-limit` 200, `memdep-cache-global-limit` 10000, `NumResultsLimit` 100 | defaults | [MemoryDependenceAnalysis.cpp#L74](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L74) |

Each cap is a named constant at its `cl::opt` default; a different value is a DEVIATE that needs a rows number.

## 2. What it replaces

| llrm today | Lines | Goes in stage |
|---|---:|---|
| `gvn.rs` (`Gvn`, `EqualCalls`, `propagated`, `numbered`, `joined`) | 685 | 1 (numbering, branch equality), 6 (`joined`) |
| `transform::subexpressions`, `equal_calls`, `forwarded` | in `transform.rs` | 1, 4 |
| `loadjoins.rs` | 478 | 4, 5 |
| `avail::forwardable_walk`, `Forward` | in `avail.rs` (1,066) | 4, if no other client remains |
| `profit::motion_price` use inside `Gvn` (crossing/avoiding double numbering) | | 5 (D8) |
| tests: `gvn_tests.rs`, `loadjoins_tests.rs` | 1,187 + 492 | kept as regression tests; deleted only where the translation's own test covers the same symptom |

`EqualCalls` (the early call numbering before `tailrec`) is not GVN in LLVM; it stays until EarlyCSE owns it (llvm-order-poc).

## 3. Owner per fact

| Fact | Owner | Used by |
|---|---|---|
| alias and mod-ref answers | basicaa-port: per-query `AliasQuery` (interface requested, not yet received) | `memdep` (every `AA.getModRefInfo` and `alias`); no alias logic in `memdep` or `gvn` |
| PHI translation of an address (`PHITransAddr`) | memoryssa-port: `mssa::phitrans::PhiTransAddr` (`translate_value`, `translate_with_insertion`, `insert_translated_sub_expr`, `needs_phi_translation_from_block`), one copy shared with its walker; its simplify step takes the same fold hook `gvn` passes; PR targeted 2026-10-20 | `memdep`, `gvn` (`collectPredecessors`, `performLoadPRE`) |
| known bits, ranges | valuetracking-port | `gvn` where LLVM calls `computeKnownBits`/`isKnownNonZero` |
| instruction simplification and constant folds | fold-poc | `gvn` where LLVM calls `simplifyInstruction`/`ConstantFoldInstruction` |
| dominators, edge dominance, critical-edge splitting | existing `cfg` shapes | `gvn` |
| order of the pass in the pipeline | llvm-order-poc | `pipeline.rs` |

Status of the two upstream dependencies is tracked in the stage-2 PR; `memdep` does not start before both interfaces exist.

## 4. Stages

Each stage is one PR (or a few), lands alone, deletes what it replaces, and carries the verification below.

| Stage | Content | Depends on |
|---|---|---|
| 1 | value table, leader map, `propagateEquality`, dead blocks, the driver; loads and calls pass through | none |
| 2 | `MemoryDependenceAnalysis`, cached, per query | AliasQuery, PHITransAddr |
| 3 | `VNCoercion` | none |
| 4 | `processLoad`, `processNonLocalLoad`, `analyzeLoadAvailability`, SSA construction; call numbering through MemDep | 1, 2, 3 |
| 5 | `performLoadPRE`, `performLoopLoadPRE` | 4 |
| 6 | `performPRE`, `performScalarPRE`, critical-edge splitting; the old pass is gone | 5 |

Stages 1 and 3 can land before the two upstream dependencies.

### Stage 1: value table, leaders, equality, the scalar pass

| LLVM | Rust | llrm today | DEVIATE |
|---|---|---|---|
| [`Expression`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L154), [`createExpr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L334), [`createCmpExpr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L372), [`createGEPExpr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L420), [`ValueTable::add`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L464), [`lookupOrAdd`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L655), [`lookup`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L743), [`lookupOrAddCmp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L756), [`lookupPtrToInt`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L764), [`clear`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L772), [`erase`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L785) | `gvn::table::ValueTable` (`Expression` hashed on opcode, type and operand value numbers) | `transform::subexpressions` keys on operands, no value-number classes | D2 `Expression` has no `Intrinsic`-attribute or `fmf` fields until MIR carries them; asserted at `create_expr` |
| [`lookupOrAddCall`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L483), [`addMemoryStateToExp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L476), [`computeLoadStoreVN`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L624), [`areCallValsEqual`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2944) | `ValueTable::lookup_or_add_call`, `compute_load_store_vn` (stage 4: they ask MemDep) | `transform::equal_calls`, `EqualCalls` pass | none; before stage 3 only the `doesNotAccessMemory` and `onlyReadsMemory` arms exist, the rest asserts unreachable |
| [`phiTranslate`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2931), [`phiTranslateImpl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2979), [`areAllValsInBB`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2923), [`eraseTranslateCacheEntry`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3061) | `ValueTable::phi_translate`, `phi_translate_impl` | none | none |
| [`LeaderMap::insert`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L807), [`LeaderMap::erase`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L819), [`findLeader`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3072), [`assignBlockRPONumber`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3105) | `gvn::leaders::LeaderMap`, `Gvn::find_leader` | dominator-order walk in `numbered` | none |
| [`propagateEquality`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3119), [`isOnlyReachableViaThisEdge`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3092) | `Gvn::propagate_equality` | `propagated` (branch conditions only) | none |
| [`processInstruction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3327), [`processBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3567), [`iterateOnFunction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3862), [`runImpl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3483), [`run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L892), [`cleanupGlobalSets`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3878), [`removeInstruction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3886), [`salvageAndRemoveInstruction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L944) | `Gvn::process_instruction`, `process_block`, `iterate_on_function`, `run_impl`, `FunctionPass::run` | `Gvn::run` | D3 `salvageAndRemoveInstruction`: MIR has no `dbg.value` use lists; the debug-location part is dropped, asserted by a body with debug info in the test |
| [`processFoldableCondBr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3997), [`addDeadBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3909), [`assignValNumForDeadCode`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L4022) | `Gvn::process_foldable_cond_br`, `add_dead_block`, `assign_val_num_for_dead_code` | none | none |
| [`processAssumeIntrinsic`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2180) | `Gvn::process_assume_intrinsic` | none | none: `llvm.assume` exists in MIR (`Intrinsic::Assume`). The `AssumptionCache` it reads is replaced by a scan of the block's assumes; DEVIATE D4 if stage 1 finds a user of the cache beyond this |
| [`processMaskedLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2878), [`findInvariantGroupValue`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2249) | none | none | D5 no masked-load and no `invariant.group` in MIR (`grep` of `llrm-mir` finds neither); asserted when the front ends construct one |

### Stage 2: MemoryDependenceAnalysis

| LLVM | Rust | llrm today | DEVIATE |
|---|---|---|---|
| [`getDependency`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L629), [`getNonLocalCallDependency`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L700), [`getCallDependencyFrom`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L192) | `memdep::MemoryDependenceResults::get_dependency`, `get_non_local_call_dependency`, `get_call_dependency_from` | none | none |
| [`getPointerDependencyFrom`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L260), [`getPointerDependencyFrom`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L288), [`getSimplePointerDependencyFrom`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L354), [`getInvariantGroupPointerDependency`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L297) | `get_pointer_dependency_from`, `get_simple_pointer_dependency_from` | `avail::forwardable_walk`, `memoryssa::may_clobber` | D5 for `getInvariantGroupPointerDependency`. Every `AA.getModRefInfo`/`alias` is one per-query `AliasQuery` from basicaa-port; no alias logic in `memdep` |
| [`getNonLocalPointerDependency`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L824), [`getNonLocalInfoForBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L891), [`getNonLocalPointerDepFromBB`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L1045), [`setNonLocalPointerDepVisited`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L1014), [`isNonLocalPointerDepVisited`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L1019) | `get_non_local_pointer_dependency`, `get_non_local_info_for_block`, `get_non_local_pointer_dep_from_bb` | none | none. `NonLocalPointerDeps` cache, `ReverseNonLocalPtrDeps`, `NonLocalDefsCache` as in LLVM; caps `BlockScanLimit` 100, `BlockNumberLimit` 200, `CacheGlobalLimit` 10000 |
| [`removeInstruction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L1521), [`removeCachedNonLocalPointerDependencies`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L1461), [`invalidateCachedPointerInfo`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L1507), [`invalidateCachedPredecessors`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L1517), [`RemoveFromReverseMap`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L96), [`verifyRemoved`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L1705) | `remove_instruction`, `remove_cached_non_local_pointer_dependencies`, `invalidate_cached_pointer_info`, `verify_removed` | none | none; `verify_removed` runs under `LLRM_CHECK_MEMDEP` |
| [`GetLocation`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L113) | `memdep::get_location` | none | none |

### Stage 3: VNCoercion

| LLVM | Rust | llrm today | DEVIATE |
|---|---|---|---|
| [`canCoerceMustAliasedValueToLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L17), [`coerceAvailableValueToLoadType`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L90) | `vncoercion::can_coerce_must_aliased_value_to_load`, `coerce_available_value_to_load_type` | forward only same type and bytes | arms for casts or types MIR lacks are numbered DEVIATEs written in stage 3 after reading `llrm-mir` types, each with an assert |
| [`analyzeLoadFromClobberingWrite`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L198), [`analyzeLoadFromClobberingStore`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L236), [`analyzeLoadFromClobberingLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L258), [`analyzeLoadFromClobberingMemInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L273) | `analyze_load_from_clobbering_*` | none | none |
| [`getValueForLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L380), [`getConstantValueForLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L401), [`getMemInstValueForLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L414), [`getConstantMemInstValueForLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L463) | `get_value_for_load`, `get_constant_value_for_load`, `get_mem_inst_value_for_load`, `get_constant_mem_inst_value_for_load` | none | constant folds through fold-poc, not a private folder |

### Stage 4: load elimination (full availability)

| LLVM | Rust | llrm today | DEVIATE |
|---|---|---|---|
| [`AvailableValue`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L204), [`AvailableValueInBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L300), [`MaterializeAdjustedValue`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1148) | `gvn::avail::AvailableValue`, `AvailableValueInBlock`, `materialize_adjusted_value` | `avail::Forward` | none |
| [`processLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2804), [`findDominatingValue`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1308), [`isLifetimeStart`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1214), [`reportMayClobberedLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1287) | `Gvn::process_load`, `find_dominating_value` | `transform::forwarded` | D7 remarks (`reportMayClobberedLoad`, `reportLoadElim`) are dropped, no remark stream |
| [`analyzeLoadAvailability`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1499), [`processNonLocalLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2059), [`processNonLocalLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2102), [`isValueFullyAvailableInBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L970), [`replaceValuesPerBlockEntry`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1086), [`eliminatePartiallyRedundantLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1626) | `analyze_load_availability`, `process_non_local_load`, `is_value_fully_available_in_block`, `construct_ssa_for_load_set` | `loadjoins::reused` (full availability part) | none; `MaxBBSpeculations` 600, `MaxNumDeps` 100 |
| [`patchAndReplaceAllUsesWith`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2241) | `patch_and_replace_all_uses_with` | metadata kept by `transform` | MIR facts that LLVM keeps as metadata (range, nonnull, align) are patched through `facts_rewrite`; asserted per fact kind |

### Stage 5: load PRE and load-in-loop PRE

| LLVM | Rust | llrm today | DEVIATE |
|---|---|---|---|
| [`performLoadPRE`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1715), [`findLoadToHoistIntoPred`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1576), [`performLoopLoadPRE`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1968) | `perform_load_pre`, `find_load_to_hoist_into_pred`, `perform_loop_load_pre` | `loadjoins::reused` with `pre`, `profit::motion_price` pricing | D8 `motion_price` pricing is not in LLVM: removed with this stage unless rows say otherwise (the pricing is then its own rows PR). Backedge split (`enable-split-backedge-in-load-pre`) off as in the default |

### Stage 6: scalar PRE and the end of the old pass

| LLVM | Rust | llrm today | DEVIATE |
|---|---|---|---|
| [`performPRE`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3799), [`performScalarPRE`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3637), [`performScalarPREInsertion`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3588), [`splitCriticalEdges`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3826), [`splitCriticalEdges`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3842) | `perform_pre`, `perform_scalar_pre`, `perform_scalar_pre_insertion`, `split_critical_edges` | `joined` with `pre` | none |

## 5. DEVIATEs

| # | What | Fact it rests on | Boundary |
|---|---|---|---|
| D1 | the `Function`/`FunctionAnalysisManager` plumbing (`run`, legacy pass) | llrm passes take a `Unit` and the `Analyses` | `FunctionPass::run` only |
| D2 | `Expression` without attribute and fast-math fields | MIR carries neither on the instructions numbered | assert in `create_expr` |
| D3 | no debug-value salvage | MIR has no debug-value use lists | test with debug info |
| D4 | `AssumptionCache` | to be confirmed in stage 1 | |
| D5 | masked loads, `invariant.group` | not in MIR | assert at the front ends |
| D7 | optimization remarks | no remark stream | |
| D8 | llrm's motion pricing | not LLVM; removed unless rows keep it | stage 5 rows |

D6 is reserved for the cast arms of `VNCoercion`, written in stage 3. D1 is the only one that does not need an assert.

## 6. Verification

Every stage:

- Oracle: the pinned `opt -passes='gvn' -S` on functions extracted from the corpora, the version printed with the result; a distro LLVM is never a reference. Stage 2 also compares `print<memdep>` of the pinned `opt`, which is registered as [`memdep`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassRegistry.def#L373). The harness is the one llvm-difftest maintains; stage 1 fixes which.
- `tools/identical.py` with every `LLRM_CHECK_*` on; a `LLRM_CHECK_MEMDEP` (cache against a fresh query) arrives with stage 2.
- A regression test per fix, seen to fail first.
- Rows, when a stage changes output: summary leads with rows total / with counters / failed / missing (`tools/rows/rowcheck.py`, not on main yet); then per-row worse counts per metric (clocks, executed instructions, memory operands, code bytes) and level, then geomean, then rows above min(gcc, clang) moved against rows/main24d. Code bytes block only at -Os; at -O2/-O3 instructions, memory operands, clocks.

Stage-specific:

1. Scalar-only inputs: the pinned `gvn` output equals llrm's on each extracted function.
2. `get_dependency` and `get_non_local_pointer_dependency` per load equal `print<memdep>`; cost per query counted against LLVM's on the same bodies.
3. Unit tests on stores of other type or offset than the load.
4. Loads: as stage 1 with loads; the three float -O3 rows with global-cell forwarding are the first rows check.
5. Load PRE and loop PRE inputs; inserted-load counts equal.
6. Whole-pass difftest; `gvn.rs`, `loadjoins.rs` and their dead helpers deleted in the same PR.

## Out of scope

- `-O1` runs GVN in llrm today; LLVM's `-O1` does not. Removing it loses `x_life` (385k against clang -O1 580k clocks), so the rows rule blocks it. It belongs to the O1-mirror re-test, once EarlyCSE, SROA/mem2reg, InstCombine, SimplifyCFG and LICM are real at their `-O1` slots.
- The MemorySSA engine of GVN, NewGVN, gvn-hoist, gvn-sink: off by default.
