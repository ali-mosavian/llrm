---
architecture-index: GVN and MemoryDependenceAnalysis as a translation of LLVM's
architecture-order: 280
---
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
| `gvn.rs` (`Gvn`, `EqualCalls`; `propagated`, `numbered`, `joined` deleted in 6) | 685 | 1 (numbering, branch equality), 6 (`joined`) |
| `transform::subexpressions`, `forwarded` (deleted in 6); `equal_calls` stays | in `transform.rs` | 1, 4 |
| `loadjoins.rs` | 478 | 4, 5 |
| `avail::forwardable_walk`, `Forward` (deleted) | in `avail.rs` (1,066) | 4 |
| `profit::motion_price` use inside `Gvn` (crossing/avoiding double numbering) | | 5 (D8) |
| tests: `gvn_tests.rs`, `loadjoins_tests.rs` | 1,187 + 492 | kept as regression tests; deleted only where the translation's own test covers the same symptom |

`EqualCalls` (the early call numbering before `tailrec`) is not GVN in LLVM; it stays until EarlyCSE owns it (llvm-order-poc).

## 3. Owner per fact

| Fact | Owner | Used by |
|---|---|---|
| alias and mod-ref answers | basicaa-port: `AliasQuery` mirroring `AAResults` at the pin under LLVM's names: `alias`, `getModRefInfo` (instruction and call forms), `getModRefInfoMask` (`pointsToConstantMemory` is a wrapper), `getMemoryEffects`, and the `BatchAAResults`/`AAQueryInfo` batch; basicaa-port writes the trait, `memdep` is written against it (no landing date yet) | `memdep` (every `AA.getModRefInfo` and `alias`); no alias logic in `memdep` or `gvn` |
| PHI translation of an address (`PHITransAddr`) | memoryssa-port: `mssa::phitrans::PhiTransAddr` (`translate_value`, `translate_with_insertion`, `insert_translated_sub_expr`, `needs_phi_translation_from_block`), one copy shared with its walker; its simplify step takes the same fold hook `gvn` passes; PR targeted 2026-10-20 | `memdep`, `gvn` (`collectPredecessors`, `performLoadPRE`) |
| known bits, ranges | valuetracking-port | `gvn` where LLVM calls `computeKnownBits`/`isKnownNonZero` |
| instruction simplification, constant folds, `ConstantFoldLoadFromConst` | fold-poc | `gvn` where LLVM calls `simplifyInstruction`/`ConstantFoldInstruction` |
| dominators, edge dominance, critical-edge splitting | existing `cfg` shapes | `gvn` |
| `AssumptionCache` | valuetracking-port: the one shared cache (`assumptions.rs`) | `gvn` (`processAssumeIntrinsic`) |
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
| [`Expression`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L154), [`createExpr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L334), [`createCmpExpr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L372), [`createGEPExpr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L420), [`ValueTable::add`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L464), [`lookupOrAdd`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L655), [`lookup`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L743), [`lookupOrAddCmp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L756), [`lookupPtrToInt`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L764), [`clear`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L772), [`erase`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L785) | `gvn::table::ValueTable` (`Expression` hashed on opcode, type and operand value numbers) | `transform::subexpressions` keys on operands, no value-number classes | none: `Expression` carries the flags field as LLVM's does (MIR has them, `opcode.rs` `Flags`) |
| [`lookupOrAddCall`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L483), [`addMemoryStateToExp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L476), [`computeLoadStoreVN`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L624), [`areCallValsEqual`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2944) | `ValueTable::lookup_or_add_call`, `compute_load_store_vn` (stage 4: they ask MemDep) | `transform::equal_calls`, `EqualCalls` pass | none; before stage 3 only the `doesNotAccessMemory` and `onlyReadsMemory` arms exist, the rest asserts unreachable |
| [`phiTranslate`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2931), [`phiTranslateImpl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2979), [`areAllValsInBB`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2923), [`eraseTranslateCacheEntry`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3061) | `ValueTable::phi_translate`, `phi_translate_impl` | none | none |
| [`LeaderMap::insert`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L807), [`LeaderMap::erase`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L819), [`findLeader`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3072), [`assignBlockRPONumber`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3105) | `gvn::leaders::LeaderMap`, `Gvn::find_leader` | dominator-order walk in `numbered` | none |
| [`propagateEquality`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3119), [`isOnlyReachableViaThisEdge`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3092) | `Gvn::propagate_equality` | `propagated` (branch conditions only) | none |
| [`processInstruction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3327), [`processBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3567), [`iterateOnFunction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3862), [`runImpl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3483), [`run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L892), [`cleanupGlobalSets`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3878), [`removeInstruction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3886), [`salvageAndRemoveInstruction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L944) | `Gvn::process_instruction`, `process_block`, `iterate_on_function`, `run_impl`, `FunctionPass::run` | `Gvn::run` | D3 `salvageAndRemoveInstruction`: MIR has no `dbg.value` use lists; the debug-location part is dropped, asserted by a body with debug info in the test |
| [`processFoldableCondBr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3997), [`addDeadBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3909), [`assignValNumForDeadCode`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L4022) | `Gvn::process_foldable_cond_br`, `add_dead_block`, `assign_val_num_for_dead_code` | none | none |
| [`processAssumeIntrinsic`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2180) | `Gvn::process_assume_intrinsic` | none | none: `llvm.assume` exists in MIR (`Intrinsic::Assume`); the assumes come from the shared `AssumptionCache` (valuetracking-port), one cache for the compiler |
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
| [`canCoerceMustAliasedValueToLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L17), [`coerceAvailableValueToLoadType`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L90) | `vncoercion::can_coerce_must_aliased_value_to_load`, `coerce_available_value_to_load_type` | forward only same type and bytes | D6 (types MIR lacks) and D9 (no constant load folder) |
| [`analyzeLoadFromClobberingWrite`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L198), [`analyzeLoadFromClobberingStore`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L236), [`analyzeLoadFromClobberingLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L258), [`analyzeLoadFromClobberingMemInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L273) | `analyze_load_from_clobbering_*` | none | none |
| [`getValueForLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L380), [`getConstantValueForLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L401), [`getMemInstValueForLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L414), [`getConstantMemInstValueForLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L463) | `get_value_for_load`, `get_constant_value_for_load`, `get_mem_inst_value_for_load`, `get_constant_mem_inst_value_for_load` | none | constant folds through fold-poc, not a private folder |

### Stage 4: load elimination (full availability)

| LLVM | Rust | llrm today | DEVIATE |
|---|---|---|---|
| [`AvailableValue`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L204), [`AvailableValueInBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L300), [`MaterializeAdjustedValue`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1148) | `gvn::avail::AvailableValue`, `AvailableValueInBlock`, `materialize_adjusted_value` | `avail::Forward` | none |
| [`processLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2804), [`findDominatingValue`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1308), [`isLifetimeStart`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1214), [`reportMayClobberedLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1287) | `Gvn::process_load`, `find_dominating_value` | `transform::forwarded` | D7 remarks (`reportMayClobberedLoad`, `reportLoadElim`) are dropped, no remark stream |
| [`analyzeLoadAvailability`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1499), [`processNonLocalLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2059), [`processNonLocalLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2102), [`isValueFullyAvailableInBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L970), [`replaceValuesPerBlockEntry`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1086), [`eliminatePartiallyRedundantLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1626) | `analyze_load_availability`, `process_non_local_load`, `is_value_fully_available_in_block`, `construct_ssa_for_load_set` | `loadjoins::reused` (full availability part) | none; `MaxBBSpeculations` 600, `MaxNumDeps` 100 |
| [`patchAndReplaceAllUsesWith`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2241) | `patch_and_replace_all_uses_with` | metadata kept by `transform` | MIR facts that LLVM keeps as metadata (range, nonnull, align) are patched through `facts_rewrite`; asserted per fact kind. IR flags are intersected on merge as `patchReplacementInstruction`/`andIRFlags` do |

### Stage 5: load PRE and load-in-loop PRE

| LLVM | Rust | llrm today | DEVIATE |
|---|---|---|---|
| [`performLoadPRE`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1715), [`findLoadToHoistIntoPred`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1576), [`performLoopLoadPRE`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1968) | `perform_load_pre`, `find_load_to_hoist_into_pred`, `perform_loop_load_pre` | `loadjoins::reused` with `pre`, `profit::motion_price` pricing | D8 `motion_price` pricing is not in LLVM: removed with this stage unless rows say otherwise (the pricing is then its own rows PR). Backedge split (`enable-split-backedge-in-load-pre`) off as in the default |

### Stage 6: scalar PRE and the end of the old pass

| LLVM | Rust | llrm today | DEVIATE |
|---|---|---|---|
| [`performPRE`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3799), [`performScalarPRE`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3637), [`performScalarPREInsertion`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3588), [`splitCriticalEdges`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3826), [`splitCriticalEdges`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3842) | `perform_pre`, `perform_scalar_pre`, `perform_scalar_pre_insertion`, `split_critical_edges` | `joined` with `pre` (deleted) | DEVIATE 23: no token types, inline asm or indirect branches in MIR; `ICF` is a block scan (22) |

## 5. DEVIATEs

| # | What | Fact it rests on | Boundary |
|---|---|---|---|
| D1 | the `Function`/`FunctionAnalysisManager` plumbing (`run`, legacy pass) | llrm passes take a `Unit` and the `Analyses` | `FunctionPass::run` only |
| D3 | no debug-value salvage | MIR has no debug-value use lists | test with debug info |
| D5 | masked loads, `invariant.group` | not in MIR | assert at the front ends |
| D6 | `VNCoercion` arms for scalable vectors, target-ext types, non-integral pointers | not in `Type` or the datalayout | none reachable |
| D15 | stage 1 asks no phi ([#2186](https://github.com/ali-mosavian/llrm/issues/2186)) to `simplifyInstruction` (temporary; oversight to rule) | a measured loss: with it x_hanoi2 -O2 +22.8% clocks, quicksort +1.5% (the loop's invariant phi simplified, its expressions merged with copies outside, a live range the allocator spills) | exit: both traced and fixed in LICM or regalloc; `gvn_scalar_tests::test_a_loops_invariant_phi_is_kept` |
| D16 | stage 1 keeps IR flags ([#2187](https://github.com/ali-mosavian/llrm/issues/2187)) in `Expression` (LLVM's has none: [GVN.cpp#L154](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L154)); temporary; oversight to rule | a measured loss: merging `sub nsw` with `sub` through [`patchReplacementInstruction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L3194) drops the `nsw`, x_mergesort -O2 +5.9% memory operands, -Os +21%; the reader of that `nsw` is not yet identified | exit: scev-port's S2 ([plan](https://github.com/ali-mosavian/llrm/pull/2150): `setNoWrapFlags`, `inferNoWrapViaConstantRanges`, `getNoWrapFlagsFromUB`) gives a reader of the lost flag what it needs, after the reader is named; `gvn_scalar_tests::test_expressions_differing_in_flags_stay_two` |
| D17 | stage 1 does not propagate a taken compare's facts about its operands (`PROPAGATE_COMPARE_FACTS`, [#2188](https://github.com/ali-mosavian/llrm/issues/2188)); temporary | a measured loss: matmul -O2 +5.7% code bytes (Nib), +2.5% (BASIC), jpeg +0.9%; in the `if i == j` arm `j := i` is done before the arm is numbered, so `add row, j` no longer equals the earlier `add row, j` | exit: the number of the replaced operand also looks up the arm's expressions; `gvn_scalar_tests::test_an_equality_taken_leaves_the_compared_operand` |
| D18 | stage 1 does not simplify GEPs ([#2199](https://github.com/ali-mosavian/llrm/issues/2199)); temporary | a measured loss: the BASIC runtime at -Os (nhstutil +10 bytes, play +9) and crc/jpeg/stft `bq32` +0.13..0.55% instructions; the pass that is sensitive to a zero GEP is not yet named | exit: that pass named and fixed; `gvn_scalar_tests::test_a_gep_of_offset_zero_is_kept` |
| D7 | optimization remarks | no remark stream | |
| D8 | llrm's motion pricing | not LLVM; removed unless rows keep it | stage 5 rows |

D9 is not a DEVIATE: `ConstantFoldLoadFromConst`/`ConstantFoldLoadFromConstPtr` ([ConstantFolding.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp)) are fold-poc's, owed. Until they land, stage 3 returns none for the arms that need them (the memcpy-from-constant-global arms and the constant-source `get_constant_*`), a gap that closes when fold-poc's functions are called. D2 and D4 are withdrawn (MIR carries the flags; the shared `AssumptionCache` is used). D1 is the only one that does not need an assert.

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

## Stage 1 measurements

[#2179](https://github.com/ali-mosavian/llrm/pull/2179), base = main built the same day.

- **Rows** (71 programs x 4 levels, gcc and clang from `main24d`): 284 total / 284 with counters / 0 failed / 0 missing. Worse: clocks, instructions, memory operands, code bytes 0 at -O1, -O2, -O3, -Os; 284 of 284 objects identical to main. Rows above min(gcc, clang), base to head, unchanged: -O1 clocks 30, instructions 48, memory operands 24, code bytes 24; -O2 32, 48, 38, 29; -O3 28, 46, 35, 39; -Os 39, 29, 21, 19.
- **Corpus** (`tools/identical.py`, defaults, every `LLRM_CHECK_*` on): 1,587 objects, 126 differ, 0 failed. Bench (`tools/bench/bench.py`, the 16 programs with differing objects, 222 cells at -O2 and -Os): 0 worse on instructions, code bytes, memory operands and clocks. `tests/run` has no counters: 5 objects smaller (up to 104 bytes), 4 larger by 1 or 2 bytes (idioms#0..#2, idiom_edges#0), traced to backend encoding in [#2212](https://github.com/ali-mosavian/llrm/issues/2212) (MIR equal until round 3, then `icmp sge x, 1` where main has `icmp sge (x-1), 0`, whose `dec; jl` reuses the flags).
- **Instrument for the traces:** `LLRM_MIR_STAGES=DIR` (there is no `tools/stages.py`); the first adjacent pair of dumps that differs names the pass.
- **Found on the way, each with a fail-first test:** the pipeline cycle between `lcssa` and phi simplification; stale registers, counted proofs and shape after the scalar pass (`tests/gvn_scalar_leaves_the_manager_true.rs`); hanoi.bas +2..4% instructions when numbering was priced before the scalar merges.
- **Temporary DEVIATEs** D15-D18, each with an issue: [#2186](https://github.com/ali-mosavian/llrm/issues/2186) no phi simplified (x_hanoi2 -O2 +22.8% clocks otherwise), [#2187](https://github.com/ali-mosavian/llrm/issues/2187) flags in the key (x_mergesort +5.9% memory operands), [#2188](https://github.com/ali-mosavian/llrm/issues/2188) compare facts not propagated (matmul +5.7% code), [#2199](https://github.com/ali-mosavian/llrm/issues/2199) no GEP simplified (consumer `loadjoins::reused`).

## Compile-time cost

Stage 1 ([#2179](https://github.com/ali-mosavian/llrm/pull/2179)) costs **+1.4%** instructions on `d_faces` -O2 (callgrind, whole compile 2,487.3 M to 2,522.2 M; geomean +0.7..0.95% over 136 files). This is **temporary**, accepted by oversight.

| | runs | IR instructions | `Gvn::run` inclusive | per unit |
|---|--:|--:|--:|--:|
| main before stage 1 | 35 | 7,920 | 228.9 M | 28,904 |
| stage 1 | 35 | 7,945 | 266.6 M | 33,557 |
| LLVM at the pin | 11 | 1,250 | 32.1 M | 25,680 |

Instrument: callgrind, inclusive of `<Gvn as FunctionPass>::run` (the analyses the pass asks the manager for are inside it), `d_faces.c` of the tree `53880fe70961`, `-m32 -mabi=sysv -march=i486 -fno-inline-functions -O2`; unit = instructions per IR instruction run over (the `[counts] pass gvn` sum), as the per-pass table in [the redesign plan](redesign-plan.md); the LLVM figure is that table's GVN row at the pin. The translated scalar part alone is 47.8 M, 6,020 per unit; the rest of the 33,557 is the old forwarding and pricing machinery.

- **Exit:** stages 4-6 delete `spill_forecast`, `loadjoins`, `motion_price` and `joined` (and the pricing that calls them, D8).
- **Target:** `Gvn::run` inclusive per unit at most 25,680 after stage 6, same instrument and scope.
- **Cycles line** (load under 4, pinned core, min of 11, A/A copy): cgscc-poc's runner, to be entered here when it lands.

## Out of scope

- `-O1` runs GVN in llrm today; LLVM's `-O1` does not. Removing it loses `x_life` (385k against clang -O1 580k clocks), so the rows rule blocks it. It belongs to the O1-mirror re-test, once EarlyCSE, SROA/mem2reg, InstCombine, SimplifyCFG and LICM are real at their `-O1` slots.
- The MemorySSA engine of GVN, NewGVN, gvn-hoist, gvn-sink: off by default.
