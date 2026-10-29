# Flat profile of `d_faces` -O2: llrm against clang

TL;DR: clang's profile is flat (the top function is 1.5% of instructions, the top 50 together 22%); llrm's is not (the top 50 are 42%). 18.0% of llrm's instructions are the allocator, `memcpy`/`memset`/`memcmp` and `Vec` growth, against 5.8% in `cc1`. In L1d misses the same split is wider: 13.9x in total, and no single `cc1` function owns more than 2.5%.

Input `d_faces.c` of qbq-llrm, `-O2 -m32 -march=i486 -fno-inline-functions`, the Borland keywords defined away. `cc1` is the driver's own command line, `valgrind --tool=callgrind`; llrm-cc is the `dist` build of main `21f5c9fc8`. Self cost summed per function name (inlined copies from different files joined). Misses from `--cache-sim=yes` (D1 32 KB, 8-way, 64 B lines); the `-Os` totals are within 1% of the `-O2` ones.

## Top 50 by self instructions

clang 24 `cc1`: 421 M in total. What the top entries do: `_int_malloc`, `malloc`, `free`: libc heap; `InterferenceCache::Entry::update` and `LiveRangeCalc::findReachingDefs`: greedy register allocation; `msort_with_tmp`: libc `qsort`; `SelectCodeCommon`: the instruction selector's matcher; `computeKnownBits*`, `BasicAAResult::aliasCheck`, `DecomposeGEPExpression`: the value-tracking and alias queries of the IR passes; `LiveRegUnits::stepBackward`, `MachineCopyPropagation::run`: machine passes.

| function | M instr | share |
|---|---:|---:|
| `_int_malloc` | 6.3 | 1.49% |
| `InterferenceCache::Entry::update` | 4.1 | 0.96% |
| `msort_with_tmp.part.0` | 3.5 | 0.84% |
| `__memset_avx2_unaligned_erms` | 3.5 | 0.82% |
| `SelectionDAGISel::SelectCodeCommon` | 2.9 | 0.68% |
| `__memcpy_avx_unaligned_erms` | 2.9 | 0.68% |
| `MachineInstr::addOperand` | 2.5 | 0.59% |
| `SmallPtrSetImplBase::insert_imp_big` | 2.4 | 0.58% |
| `computeKnownBitsFromOperator` | 2.2 | 0.52% |
| `LiveRegUnits::stepBackward` | 2.2 | 0.52% |
| `xxh3_64bits` | 2.1 | 0.50% |
| `MachineCopyPropagation::run` | 2.1 | 0.50% |
| `DataLayout::getTypeAllocSize` | 2.1 | 0.49% |
| `_dl_relocate_object_no_relro` | 2.0 | 0.49% |
| `BasicAAResult::aliasCheck` | 2.0 | 0.47% |
| `BasicAAResult::DecomposeGEPExpression` | 1.9 | 0.46% |
| `computeKnownBits` | 1.9 | 0.46% |
| `free` | 1.8 | 0.44% |
| `mayBeAccessToSubobjectOf` | 1.8 | 0.44% |
| `IDFCalculatorBase<BasicBlock, false>::calculate` | 1.7 | 0.41% |
| `getUnderlyingObject` | 1.7 | 0.41% |
| `SelectionDAG::Combine` | 1.7 | 0.41% |
| `malloc` | 1.7 | 0.39% |
| `computeKnownBitsFromContext` | 1.6 | 0.39% |
| `LiveRangeCalc::findReachingDefs` | 1.6 | 0.38% |
| `LiveRangeUpdater::add` | 1.5 | 0.36% |
| `LiveRange::extendInBlock` | 1.5 | 0.36% |
| `LiveIntervalUnion::Query::collectInterferingVRegs` | 1.4 | 0.34% |
| `SlotIndexes::getInstructionIndex` | 1.4 | 0.34% |
| `TargetLowering::SimplifyDemandedBits` | 1.4 | 0.34% |
| `UniquingSet<SDNode, SDNodeKeyInfo>::lookup` | 1.4 | 0.34% |
| `MemoryDependenceResults::getSimplePointerDependencyFrom` | 1.4 | 0.34% |
| `MachineLICMImpl::run` | 1.4 | 0.33% |
| `LiveRange::find` | 1.4 | 0.33% |
| `IntervalMap<SlotIndex, LiveInterval const*, 8u, IntervalMapInfo<SlotIndex> >::` | 1.4 | 0.32% |
| `SpillPlacement::update` | 1.3 | 0.32% |
| `AttributeList::hasAttributeAtIndex` | 1.3 | 0.31% |
| `Value::stripPointerCastsForAliasAnalysis` | 1.3 | 0.30% |
| `__mempcpy_avx_unaligned_erms` | 1.3 | 0.30% |
| `InstCombinerImpl::run` | 1.2 | 0.30% |
| `LiveRange::overlaps` | 1.2 | 0.30% |
| `_int_free_chunk` | 1.2 | 0.29% |
| `SpillPlacement::activate` | 1.2 | 0.28% |
| `IntervalMap<SlotIndex, LiveInterval const*, 8u, IntervalMapInfo<SlotIndex> >::` | 1.1 | 0.27% |
| `ScalarEvolution::getAddExpr` | 1.1 | 0.27% |
| `SmallPtrSetImplBase::Grow` | 1.1 | 0.27% |
| `InstCombinerImpl::prepareWorklist` | 1.1 | 0.26% |
| `SelectionDAG::computeKnownBits` | 1.1 | 0.26% |
| `ScalarEvolution::getRangeRef` | 1.1 | 0.26% |
| `Type::getPrimitiveSizeInBits` | 1.1 | 0.25% |

llrm-cc: 2,874 M. What the top entries do: the `mi_*` rows are mimalloc; `escapes`, `MemRef::of`, `walked_from`, `structure_of`, `exposed_frames`, `point_values` are the alias and memory-SSA facts; `View::forecast` is the spill forecast; `Function::successors` builds a `Vec` of a block's successors.

| function | M instr | share | what it does |
|---|---:|---:|---|
| `__memcpy_avx_unaligned_erms` | 128.5 | 4.47% | copies from `Vec` growth and clones |
| `mi_free` | 82.3 | 2.86% | mimalloc free |
| `mi_theap_malloc_aligned` | 75.3 | 2.62% | mimalloc malloc |
| `analysis::alias::escapes` | 61.2 | 2.13% | whether a pointer escapes the function |
| `MemRef::of` | 51.6 | 1.79% | the memory reference of an access |
| `<alloc::raw_vec::RawVecInner>::finish_grow` | 49.9 | 1.74% | `Vec` growth |
| `MemorySSA::walked_from` | 49.6 | 1.73% | memory SSA clobber walk |
| `_mi_page_malloc_zero` | 43.9 | 1.53% | mimalloc page free list |
| `View::forecast` | 33.7 | 1.17% | what fitting the function spills |
| `analysis::memoryssa::structure_of` | 26.6 | 0.92% | builds the memory SSA structure |
| `Function::successors` | 26.1 | 0.91% | a block's successors, as a new `Vec` |
| `mi_malloc_aligned` | 24.2 | 0.84% | mimalloc malloc |
| `__memcmp_avx2_movbe` | 22.7 | 0.79% | byte comparison (printed-body history, keys) |
| `Opcode::eq` | 21.4 | 0.74% |  |
| `mir::forest::forest` | 21.3 | 0.74% | loop forest of a CFG |
| `analysis::memory::exposed_frames` | 21.3 | 0.74% | allocas whose address is exposed |
| `<core::backend::allocate::LiveRows>::sets` | 20.8 | 0.72% | register-allocation live sets |
| `<alloc::collections::btree::map::BTreeMap<mir::module::BlockId, alloc::collect` | 19.8 | 0.69% |  |
| `<alloc::collections::btree::map::BTreeMap<i64, alloc::collections::btree::set_` | 19.6 | 0.68% |  |
| `analysis::constant_cycles::propagated` | 18.5 | 0.64% | constants around loop cycles |
| `_mi_theap_realloc_zero` | 18.1 | 0.63% |  |
| `<core::backend::allocate::Facts>::of` | 17.8 | 0.62% | register-allocation facts |
| `analysis::alias::point_values` | 17.4 | 0.61% | points-to values of pointers |
| `<core::backend::splitkit::Occupied>::interference` | 16.9 | 0.59% |  |
| `analysis::memory::classes_may_alias` | 16.4 | 0.57% | alias of two object classes |
| `DominatorTree::new` | 16.2 | 0.56% | dominator tree |
| `core::backend::splitkit::crossings_in` | 16.1 | 0.56% |  |
| `__memset_avx2_unaligned_erms` | 15.7 | 0.55% |  |
| `core::backend::liveness::live_into` | 14.9 | 0.52% |  |
| `MemorySSA::clobbered` | 14.4 | 0.50% |  |
| `support::graph::predecessors::<analysis::cfg::Block>` | 13.7 | 0.48% |  |
| `analysis::alias::initialized::{closure#5}` | 13.5 | 0.47% |  |
| `Offsets::stepped` | 13.2 | 0.46% |  |
| `<indexmap::map::IndexMap<u32, alloc::sync::Arc<core::model::lir::Insn>, core::` | 12.8 | 0.44% |  |
| `<alloc::raw_vec::RawVec<mir::types::TypeId>>::grow_one` | 12.5 | 0.43% |  |
| `analysis::regions::foreign` | 12.3 | 0.43% |  |
| `core::slice::sort::unstable::quicksort::quicksort::<` | 12.3 | 0.43% |  |
| `Given::holds` | 12.2 | 0.43% |  |
| `<indexmap::map::IndexMap<i64, (usize, usize), core::hash::BuildHasherDefault<r` | 12.1 | 0.42% |  |
| `transforms::promote::_available::{closure#8}` | 11.7 | 0.41% |  |
| `<alloc::collections::btree::map::BTreeMap<usize, alloc::collections::btree::se` | 11.6 | 0.40% |  |
| `<indexmap::map::IndexMap<mir::module::InstId, core::option::Option<alloc::coll` | 11.6 | 0.40% |  |
| `support::env_set` | 11.3 | 0.39% |  |
| `<alloc::collections::btree::map::IntoIter<core::option::Option<mir::context::C` | 11.2 | 0.39% |  |
| `Problem::total_of` | 11.1 | 0.39% |  |
| `analysis::induction::_recurrences` | 11.0 | 0.38% |  |
| `<core::backend::liveness::Effect>::live_before` | 10.8 | 0.37% |  |
| `<transforms::spill::Ranked<transforms::lsr::Resident>>::add::<transforms::lsr:` | 10.3 | 0.36% |  |
| `alloc::fmt::format::format_inner` | 10.2 | 0.36% | `format!` |
| `<core::iter::adapters::flatten::FlatMap<core::iter::adapters::enumerate::Enume` | 10.2 | 0.36% |  |

## Top 25 by L1d misses

clang 24 `cc1`: 3.08 M in total (instructions: 415 M).

| function | M D1 misses | share |
|---|---:|---:|
| `__memset_avx2_unaligned_erms` | 0.08 | 2.5% |
| `_int_malloc` | 0.07 | 2.4% |
| `__memcpy_avx_unaligned_erms` | 0.06 | 1.8% |
| `new ` | 0.05 | 1.7% |
| `_dl_relocate_object_no_relro` | 0.04 | 1.3% |
| `InterferenceCache::Entry::update` | 0.04 | 1.2% |
| `LiveRangeCalc::findReachingDefs` | 0.04 | 1.2% |
| `:memcpy` | 0.03 | 1.1% |
| `new ` | 0.03 | 1.1% |
| `Instruction::successors` | 0.03 | 1.1% |
| `SplitAnalysis::calcLiveBlockInfo` | 0.03 | 1.0% |
| `InstCombinerImpl::prepareWorklist` | 0.03 | 0.9% |
| `removeUnreachableBlocks` | 0.03 | 0.9% |
| `malloc_consolidate` | 0.03 | 0.8% |
| `SlotIndexes::getInstructionIndex` | 0.03 | 0.8% |
| `unlink_chunk.isra.0` | 0.02 | 0.8% |
| `InstCombinerImpl::run` | 0.02 | 0.7% |
| `ScalarEvolution::visitAndClearUsers` | 0.02 | 0.6% |
| `:memcpy` | 0.02 | 0.6% |
| `` | 0.02 | 0.6% |
| `` | 0.02 | 0.6% |
| `LowerDbgDeclare` | 0.02 | 0.6% |
| `malloc` | 0.02 | 0.6% |
| `` | 0.02 | 0.6% |
| `free` | 0.02 | 0.5% |

llrm-cc: 42.7 M in total (instructions: 2,903 M).

| function | M D1 misses | share |
|---|---:|---:|
| `__memcpy_avx_unaligned_erms` | 3.89 | 9.1% |
| `_mi_page_malloc_zero` | 2.53 | 5.9% |
| `Function::successors` | 0.95 | 2.2% |
| `analysis::alias::escapes` | 0.94 | 2.2% |
| `hir::verify::_facts` | 0.90 | 2.1% |
| `__memcmp_avx2_movbe` | 0.86 | 2.0% |
| `mi_free` | 0.83 | 1.9% |
| `<core::backend::allocate::Facts>::of` | 0.80 | 1.9% |
| `<core::analysis::graph::Graph>::of` | 0.78 | 1.8% |
| `MemorySSA::walked_from` | 0.71 | 1.7% |
| `analysis::memory::exposed_frames` | 0.70 | 1.6% |
| `analysis::alias::congruences_with` | 0.67 | 1.6% |
| `MemRef::of` | 0.54 | 1.3% |
| `__memset_avx2_unaligned_erms` | 0.46 | 1.1% |
| `core::backend::splitkit::_next_value_following` | 0.45 | 1.0% |
| `analysis::constant_cycles::propagated` | 0.43 | 1.0% |
| `Opcode::eq` | 0.43 | 1.0% |
| `core::backend::allocate::_allocated` | 0.41 | 1.0% |
| `analysis::alias::point_values` | 0.39 | 0.9% |
| `analysis::memoryssa::structure_of` | 0.33 | 0.8% |
| `mi_theap_umalloc` | 0.27 | 0.6% |
| `Unit::intrinsic` | 0.25 | 0.6% |
| `DominatorTree::new` | 0.24 | 0.6% |
| `Pressure::updated` | 0.24 | 0.6% |
| `core::backend::masm::_roots_of_values` | 0.23 | 0.5% |

`-Os`: 42.7 M against 3.04 M; the leaders are the same (`memcpy`, `_mi_page_malloc_zero`, `Function::successors`).

## Miss ownership by phase and pass

Inclusive D1 misses (analyses a pass asks for are inside it):

| owner | M instr | M D1 misses | misses per k-instr |
|---|---:|---:|---:|
| whole compile | 2,903 | 42.7 | 14.7 |
| optimizer (`driver::optimized`) | 1,856 | 23.9 | 12.8 |
| backend (`assemble::assembled_by`) | 922 | 16.3 | 17.7 |
| of which register allocation | 434 | 9.4 | 21.7 |
| `fold` | 267 | 3.9 | 14.7 |
| `gvn` | 249 | 2.9 | 11.5 |
| `lsr` | 149 | 2.3 | 15.6 |
| `hoist` | 168 | 1.9 | 11.1 |
| `decide` | 135 | 1.7 | 12.4 |

By structure, self misses added up: allocator and copying 23.7% (10.1 M), memory-fact tables (`alias`, `memory`, `memoryssa`, `avail`, `regions`, `pointerfacts`) 13.6%, register-allocation tables 8.7%, IR walks (`Function::successors`, `Graph::of`, dominators, `Opcode::eq`) 7.1%, `hir::verify` 2.2%. No LL miss problem: 0.67 M LL misses against 0.25 M; the working set fits the last level, so the cost is L1 and L2 traffic.

## Proposed change

1. Allocation and copying (518 M, 18.0% of instructions): the three changes on the [allocation page](allocation.md#proposed-change).
2. Alias and memory-SSA queries at the top of the profile: `alias::escapes` 61.2 M self, `MemRef::of` 51.6 M, `MemorySSA::walked_from` 49.6 M, `memoryssa::structure_of` 26.6 M, `exposed_frames` 21.3 M. Answer them from per-body-version tables built on first use (lazy memory facts, keep-analyses) and the walker tables (greedy-port). Saving: unmeasured; bound 210 M, 7.3%, before the callers' own work.
3. Bookkeeping: the printed-body history is replaced by a digest in [#1931](https://github.com/ali-mosavian/llrm/pull/1931) (merged); the environment lookup per step (`env_set`, 11.4 M) in [#2007](https://github.com/ali-mosavian/llrm/pull/2007) (open); `Stamp` (49.1 M, 1.7%) and the verifiers (22 M) have no change proposed yet.

Ranked list: [proposals](proposals.md).

Open: which of the memory-fact tables can be dense ids or bit sets, and what that saves in misses ([#1994](https://github.com/ali-mosavian/llrm/issues/1994)).
