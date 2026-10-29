---
architecture-index: LLVM's MemorySSA walker against llrm's frontier walk
architecture-order: 340
---
# LLVM's MemorySSA clobber walker against llrm's frontier walk

TL;DR: LLVM answers a memory query from a field the access keeps, and walks at most 100 stores when it does not; llrm walks from the site for every query, 4,787 on d_faces `-O2` at 27,750 instructions each against LLVM's 286 at about 11,000.

## LLVM

`getClobberingMemoryAccessBase` returns the clobbering access a use already holds (`isOptimized`) and otherwise walks up at most `UpwardWalkLimit` stores and phis, which starts at `memssa-check-limit` (100, [`MemorySSA.cpp#L76-L79`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L76-L79)); the answer is stored back with `setOptimized` [`llvm/lib/Analysis/MemorySSA.cpp#L2556-L2593`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L2556-L2593):

```cpp
  if (StartingAccess->isOptimized()) {
    if (!SkipSelf || !isa<MemoryDef>(StartingAccess))
      return StartingAccess->getOptimized();
    IsOptimized = true;
  }

  const Instruction *I = StartingAccess->getMemoryInst();
  // We can't sanely do anything with a fence, since they conservatively clobber
  // all memory, and have no locations to get pointers from to try to
  // disambiguate.
  if (!isa<CallBase>(I) && I->isFenceLike())
    return StartingAccess;

  UpwardsMemoryQuery Q(I, StartingAccess);

  if (isUseTriviallyOptimizableToLiveOnEntry(BAA, I)) {
    MemoryAccess *LiveOnEntry = MSSA->getLiveOnEntryDef();
    StartingAccess->setOptimized(LiveOnEntry);
    return LiveOnEntry;
  }

  MemoryAccess *OptimizedAccess;
  if (!IsOptimized) {
    // Start with the thing we already think clobbers this location
    MemoryAccess *DefiningAccess = StartingAccess->getDefiningAccess();

    // At this point, DefiningAccess may be the live on entry def.
    // If it is, we will not get a better result.
    if (MSSA->isLiveOnEntryDef(DefiningAccess)) {
      StartingAccess->setOptimized(DefiningAccess);
      return DefiningAccess;
    }

    OptimizedAccess =
        Walker.findClobber(BAA, DefiningAccess, Q, UpwardWalkLimit);
    StartingAccess->setOptimized(OptimizedAccess);
  } else
    OptimizedAccess = StartingAccess->getOptimized();
```

## llrm

`nearest_writes` is the walker's one capped entry: `limit` stops the walk once more than `MAX_NEAREST_WRITES` (100, [`memoryssa.rs#L992-L996`](https://github.com/ali-mosavian/llrm/blob/21f5c9fc8e67b5bf405cf9ecdaf65247a26c0450/crates/opt/llrm-analysis/src/memoryssa.rs#L992-L996)) writes are found; the other entry points (`clobbers`, `clobbers_ignoring_invariance`) walk to the end, and nothing is stored on the access [`crates/opt/llrm-analysis/src/memoryssa.rs#L1069-L1081`](https://github.com/ali-mosavian/llrm/blob/21f5c9fc8e67b5bf405cf9ecdaf65247a26c0450/crates/opt/llrm-analysis/src/memoryssa.rs#L1069-L1081):

```rust
    /// `clobbers_ignoring_invariance`, stopped once more than
    /// `MAX_NEAREST_WRITES` are found: a set longer than that is incomplete,
    /// and its length is all a caller reads of it.
    pub fn nearest_writes(
        &self,
        site: InstId,
        memory: &MemRef,
    ) -> BTreeSet<usize> {
        self.limit.set(Some(MAX_NEAREST_WRITES));
        let found = self.frontier_for(site, memory, None, None, None, false);
        self.limit.set(None);
        found
    }
```

## Divergence

The cap is the same size and idea, on one of three entry points. The difference is the cache: LLVM's `OptimizeUses` gives each use its clobbering access once at build and keeps it across edits, so a repeated query is a field read; llrm walks from the site in every function version with per-walk caches that die with it. Of the 4,787 queries 65% repeat a key an earlier version asked and 95.6% of those give the same answer, but a cache keyed on (site, cell, kind) covers 1.4% of the compile at most, below the bar, so it was measured and not built (compile-time-spec section 8).

## Measured

| | LLVM (clang 24, cc1) | llrm |
|---|---:|---:|
| walker queries | 286 `getClobberingMemoryAccess` | 4,787 frontier queries over 54 function versions |
| per query | about 11,000 instructions (3.2 M in all) | 27,750 instructions (133 M in all) |
| walker, inclusive | `ClobberWalker::findClobber` 3.05 M (0.73%) | `frontier_for` 140.5 M (4.36%) |
| structure build | `MemorySSAAnalysis::run` 2.47 M (0.60%), 27 builds | `Accesses::run` 194.1 M (6.02%), 99 builds |
| distinct keys | not counted | 1,675 of 4,787 queries |

Input `render/d_faces.c`, `-m32 -march=i486 -fno-inline-functions -O2`. LLVM: callgrind on `clang -cc1` of d1106deb71cc (total 415.4 M); query counts from llvm-measured. llrm: callgrind on the release `llrm-cc` of 21f5c9fc8 (total 3,224 M); query and key counts from the walker-cache entry of compile-time-spec section 8 (dist build).

## Proposed change

- **Files**: `crates/opt/llrm-analysis/src/memoryssa.rs` (`MemorySSA::walked_from`: the candidate loop of the nearest-write index branch, `slot`, `clobbered`); callers `consts::load_providers` and `provider_of`.
- **What**: take `slot(memory)` out of the loop (it is invariant), memoise `clobbered` per (slot, candidate) so a later load of the same cell restarts at the last clobber, and add a count cap (LLVM's 100) only if a corpus counter shows no query passes it.
- **Expected saving**: on the mem ladder at N=1600 the candidate loop is 1,257,255 iterations (74,444 at N=400, 16.9x for 4x), `slot` 640.6 M and `clobbered` 111 M of `frontier_for` 1,102 M; after the hoist `frontier_for` is 422 M with 1.28 M iterations left, so the memo is the rest of a slope; its share is unmeasured. A cache keyed on (site, cell, kind) across versions covers at most 1.4% of the `d_faces` compile.
- **Status**: the `slot` hoist and paged walker tables are #2011 (greedy-port); the memo is unbuilt; the cross-version cache is measured and not kept ([compile-time-spec section 8](../../architecture/compile-time-spec.md), "A cache over the memory walker").
