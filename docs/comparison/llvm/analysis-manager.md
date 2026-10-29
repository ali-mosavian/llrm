---
architecture-index: LLVM's analysis manager against llrm's `Analyses`
architecture-order: 330
---
# LLVM's analysis manager against llrm's `Analyses`

TL;DR: dropping analyses costs the same in both (2.6 M instructions against 2.9 M on d_faces `-O2`); what differs is the price of getting one back: llrm rebuilds whole-function maps 66 to 202 times over 11 functions at 0.1 to 1.1 M each, LLVM's DominatorTree is built 55 times for 0.5 M in all.

## LLVM

After each pass `PassManager::run` calls `AM.invalidate(IR, PassPA)` with what the pass said it kept, and a pass that changed nothing returns `PreservedAnalyses::all()`; `invalidate` returns at once when everything is preserved, and otherwise asks each held result whether it survives [`llvm/include/llvm/IR/PassManagerImpl.h#L169-L207`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/PassManagerImpl.h#L169-L207):

```cpp
inline void AnalysisManager<IRUnitT, ExtraArgTs...>::invalidate(
    IRUnitT &IR, const PreservedAnalyses &PA) {
  // We're done if all analyses on this IR unit are preserved.
  if (PA.allAnalysesInSetPreserved<AllAnalysesOn<IRUnitT>>())
    return;

  // Track whether each analysis's result is invalidated in
  // IsResultInvalidated.
  SmallDenseMap<AnalysisKey *, bool, 8> IsResultInvalidated;
  Invalidator Inv(IsResultInvalidated, AnalysisResults);
  auto ResultsListI = AnalysisResultLists.find(&IR);
  if (ResultsListI == AnalysisResultLists.end())
    return;
  AnalysisResultListT &ResultsList = ResultsListI->second;
  for (auto &AnalysisResultPair : ResultsList) {
    // This is basically the same thing as Invalidator::invalidate, but we
    // can't call it here because we're operating on the type-erased result.
    // Moreover if we instead called invalidate() directly, it would do an
    // unnecessary look up in ResultsList.
    AnalysisKey *ID = AnalysisResultPair.first;
    auto &Result = *AnalysisResultPair.second;

    auto IMapI = IsResultInvalidated.find(ID);
    if (IMapI != IsResultInvalidated.end())
      // This result was already handled via the Invalidator.
      continue;

    // Try to invalidate the result, giving it the Invalidator so it can
    // recursively query for any dependencies it has and record the result.
    // Note that we cannot reuse 'IMapI' here or pre-insert the ID, as
    // Result.invalidate may insert things into the map, invalidating our
    // iterator.
    bool Inserted =
        IsResultInvalidated.insert({ID, Result.invalidate(IR, PA, Inv)}).second;
    (void)Inserted;
    assert(Inserted && "Should never have already inserted this ID, likely "
                       "indicates a cycle!");
  }

```

## llrm

`Analyses::drop_unpreserved` is the same step: it drops each held result its pass did not preserve, unless the result is `current` (derived from the function as it stands, the mark-equal keep). An incremental result is moved to `kept` instead of dropped, so the next `get` can bring it up to date from the change log [`crates/ir/llrm-mir/src/passes.rs#L1491-L1524`](https://github.com/ali-mosavian/llrm/blob/21f5c9fc8e67b5bf405cf9ecdaf65247a26c0450/crates/ir/llrm-mir/src/passes.rs#L1491-L1524):

```rust
    /// Drops what `preserved` does not keep, except (`current`) a result
    /// derived from `function` as it stands: an edit that was made and put
    /// back, or made before the result was asked for, left it true. Not one
    /// the pass names as given up.
    fn drop_unpreserved(
        &mut self,
        preserved: &PreservedAnalyses,
        current: Option<&Function>,
    ) {
        let outer = Rc::clone(&self.outer);
        let attributes = current.map(Function::attributes_stamp);
        self.attributes = attributes;
        let gone: Vec<TypeId> = self
            .cache
            .iter()
            .filter(|(key, entry)| {
                // A result a pass names as given up goes whatever the log says.
                !entry.preserved(preserved)
                    && (preserved.dropped.contains(*key)
                        || !current
                            .zip(attributes)
                            .is_some_and(|(function, attributes)| entry.current(function, attributes, &outer)))
            })
            .map(|(key, _)| *key)
            .collect();
        for key in gone {
            let entry = self.cache.remove(&key).expect("held");
            if entry.incremental() {
                self.kept.insert(key, entry);
            } else if why() {
                self.evicted.insert(key, entry);
            }
        }
    }
```

## Divergence

The step is the same and equally cheap; the cost is behind it. A dropped llrm result is a whole-function map (`bounded`, `annotated`, `through-memory`), so each rebuild is 0.1 to 1.1 M and comes once per function version: 66 to 202 builds against LLVM's 27 to 55. `kept` and the mark-equal keep soften it (asks hit what is held 36% for `bounded` to 91% for `shape`), but llrm's analysis runs are 651 M, about a quarter of its compile, against 5.3 M (1.3%) for LLVM's `getResultImpl`.

## Measured

| | LLVM (clang 24, cc1) | llrm |
|---|---:|---:|
| invalidation, inclusive | `AnalysisManager<Function>::invalidate` 2.63 M (0.63%) | `drop_unpreserved` 2.85 M (0.09%) |
| analyses computed through the manager | `getResultImpl` 5.30 M (1.28%) | 2,953 runs, 651 M own (llvm-measured) |
| DominatorTree | 55 builds, 0.48 M in all | `dominators` 11.7 M inclusive of its `get` |
| MemorySSA / `accesses` | 27 builds, 2.5 M | 99 builds, 12.7 M own, 194 M inclusive of the `run` |
| `registers`, `counted`, `bounded` | no counterpart | 202 builds 32.7 M, 143 builds 34.3 M, 135 builds 103.6 M |
| `points-to`, `annotated`, `through-memory` | no counterpart (AAManager 44 builds) | 118 builds 48.0 M, 116 builds 78.9 M, 66 builds 75.4 M |

Input `render/d_faces.c`, `-m32 -march=i486 -fno-inline-functions -O2`, 11 functions. LLVM: `valgrind --tool=callgrind` on `clang -cc1` of d1106deb71cc, total 415.4 M. llrm: callgrind on the release `llrm-cc` of 21f5c9fc8, total 3,224 M; build counts and own M from `LLRM_DEBUG=time`.

## Proposed change

- **Files**: `crates/ir/llrm-mir/src/passes.rs` (`Analyses::get`, `drop_unpreserved`); the analyses whose rebuild is a whole-function map (`Annotated` in `manager.rs`, owned by keep-analyses).
- **What**: bring an analysis up to date from the change log instead of rebuilding it, as `Registers` and `Counted` are; an exact `Annotated::update` from the `Substituted` entries (re-annotate only the accesses whose operands changed).
- **Expected saving**: unmeasured. Bound: the rebuilds that return what was held were measured at 191 M of d_faces `-O2` by the analysis's average cost per build, and two cheap exact keeps (`Homes` reporting no change; a kept result with an empty change log despite a different `Outer`) saved 0.00% and 0.04%; the average overstated the cheap builds 20-fold. 94% of the `bounded` and `exposed-frames` reruns follow edits to a branch, phi, memory operation or gep.
- **Status**: measured and not kept for the cheap keeps ([compile-time-spec section 8](../../architecture/compile-time-spec.md), "Analysis rebuilds that return what was held"); `Annotated::update` is a proposal with keep-analyses.
