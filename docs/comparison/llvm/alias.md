---
architecture-index: LLVM's BasicAA against llrm's `regions::overlapping`
architecture-order: 350
---
# LLVM's BasicAA against llrm's `regions::overlapping`

TL;DR: LLVM answers alias on demand from the pointers, looking through at most 10 steps and caching each pair; llrm answers from a provenance it built for every access, with no cache, and asks 10 times as often: 104,236 overlap queries (58.8 M) against 10,624 alias queries (19.6 M) on d_faces `-O2`.

## LLVM

`aliasCheck` caps recursion at depth 512, then looks the pair up in `AAQI.AliasCache` before climbing the use-def chain; the underlying object of each pointer is found on demand by `getUnderlyingObject(V, MaxLookupSearchDepth)`, 10 steps ([`ValueTracking.h#L52`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueTracking.h#L52-L52)) [`llvm/lib/Analysis/BasicAliasAnalysis.cpp#L1684-L1720`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BasicAliasAnalysis.cpp#L1684-L1720):

```cpp
  // for recursive queries. For this reason, this limit is chosen to be large
  // enough to be very rarely hit, while still being small enough to avoid
  // stack overflows.
  if (AAQI.Depth >= 512)
    return AliasResult::MayAlias;

  // Check the cache before climbing up use-def chains. This also terminates
  // otherwise infinitely recursive queries. Include MayBeCrossIteration in the
  // cache key, because some cases where MayBeCrossIteration==false returns
  // MustAlias or NoAlias may become MayAlias under MayBeCrossIteration==true.
  AAQueryInfo::LocPair Locs({V1, V1Size, AAQI.MayBeCrossIteration},
                            {V2, V2Size, AAQI.MayBeCrossIteration});
  const bool Swapped = V1 > V2;
  if (Swapped)
    std::swap(Locs.first, Locs.second);
  const auto &Pair = AAQI.AliasCache.try_emplace(
      Locs, AAQueryInfo::CacheEntry{AliasResult::NoAlias, 0});
  if (!Pair.second) {
    auto &Entry = Pair.first->second;
    if (!Entry.isDefinitive()) {
      // Remember that we used an assumption. This may either be a direct use
      // of an assumption, or a use of an entry that may itself be based on an
      // assumption.
      ++AAQI.NumAssumptionUses;
      if (Entry.isAssumption())
        ++Entry.NumAssumptionUses;
    }
    // Cache contains sorted {V1,V2} pairs but we should return original order.
    auto Result = Entry.Result;
    Result.swap(Swapped);
    return Result;
  }

  int OrigNumAssumptionUses = AAQI.NumAssumptionUses;
  unsigned OrigNumAssumptionBasedResults = AAQI.AssumptionBasedResults.size();
  AliasResult Result =
      aliasCheckRecursive(V1, V1Size, V2, V2Size, AAQI, O1, O2);
```

## llrm

`overlapping` answers from the `MemRef` of each access: its provenance (objects, restrict sets) is built once per access by `MemRef::of`, and a query compares two of them, then their displacements, then `may_alias`; it keeps no cache of pairs and no depth bound [`crates/opt/llrm-analysis/src/regions.rs#L392-L430`](https://github.com/ali-mosavian/llrm/blob/21f5c9fc8e67b5bf405cf9ecdaf65247a26c0450/crates/opt/llrm-analysis/src/regions.rs#L392-L430):

```rust
/// Whether a write through `other` could land on `one`.
///
/// Provenance answers the symbolic
/// part; when two references keep one root and index, their byte intervals
/// answer the remaining displacement question.
pub fn overlapping(
    one: &MemRef,
    other: &MemRef,
    known: Option<&BTreeMap<ValueId, Interval>>,
    other_known: Option<&BTreeMap<ValueId, Interval>>,
    program: Option<&ProgramProxy>,
) -> Result<bool, RegionError> {
    if typed_apart(one, other) || foreign_apart(one, other, known, other_known, program) {
        return Ok(false);
    }
    // Canonical references carry their own object identity. Keep their base
    // value available to the canonical range query; the covering rewrite
    // below erases it after widening the address to a byte hull.
    if one.provenance.is_some() && other.provenance.is_some() {
        return match _displaced(one, other) {
            Some(apart) => Ok(!apart),
            None => may_alias(one, other, known, other_known, program),
        };
    }
    if (_unescaped(one) && _through_pointer(other)) || (_unescaped(other) && _through_pointer(one)) {
        return Ok(false);
    }
    let have_facts = known.is_some_and(|facts| !facts.is_empty()) || other_known.is_some_and(|facts| !facts.is_empty());
    let empty = BTreeMap::new();
    let (covered, other_covered) = if have_facts {
        (covering(one, known.unwrap_or(&empty)), covering(other, other_known.unwrap_or(&empty)))
    } else {
        (Cow::Borrowed(one), Cow::Borrowed(other))
    };
    if let Some(apart) = _displaced(&covered, &other_covered) {
        return Ok(!apart);
    }
    may_alias(one, other, known, other_known, program)
}
```

## Divergence

The two buy the same ruling-out by opposite means: LLVM pays at the query (a bounded walk, then a cache hit) and llrm pays at the access (`MemRef::of`, 185,644 calls, 109.4 M) and then answers cheaply (564 instructions a query against LLVM's 1,800). llrm has no cache of pairs and is asked 10 times as often; how many of the 104,236 queries repeat a pair is not measured.

## Measured

| | LLVM (clang 24, cc1) | llrm |
|---|---:|---:|
| alias queries | 10,624 `BasicAAResult::alias` calls | 104,236 `overlapping` calls |
| inclusive | 19.6 M (4.71%); `AAResults::alias` 20.1 M | `overlapping` 58.8 M (1.82%) |
| per query | about 1,800 | about 564 |
| per-access setup | none (on demand) | `MemRef::of` 185,644 calls, 109.4 M (3.39%) |
| whole alias tower | AAManager 44 builds | `Annotated` 297.6 M inclusive (9.23%), `points_to` 150.5 M (4.67%) |

Input `render/d_faces.c`, `-m32 -march=i486 -fno-inline-functions -O2`. LLVM: callgrind on `clang -cc1` of d1106deb71cc, total 415.4 M, calls summed over callers. llrm: callgrind on the release `llrm-cc` of 21f5c9fc8, total 3,224 M.

## Proposed change

- **Files**: `crates/opt/llrm-analysis/src/avail.rs` (`Solve::reaching`, `Stored::related`), `crates/opt/llrm-analysis/src/regions.rs` (`overlap_frame`).
- **What**: an access reaches the cells of its own object and frame whose bytes meet its own (a sorted byte index), not every cell of its object; and the cells of one address, which have the same byte keys, share the sets of what covers them.
- **Expected saving**: `dse` on the ladder at N=1600: mem 1,907 to 179 M and the distinct-offset shape 3,395 to 162 M (reach sets), arith 818 to 52 M (twin cells); every shape goes from 3.5x-4.0x to 2.0x per doubling. On `d_faces` +0.19% instructions for the reach index at `-O2`, +0.01% to +0.03% for the twins; objects byte-identical.
- **Status**: PR #2013 (reach sets) and PR #2015 (twin cells); `LLRM_CHECK_REACHED` and `LLRM_CHECK_CLOBBERS` hold them to the dense forms over the 1,583-object corpus. A cache of alias pairs (LLVM's `AliasCache`) is unmeasured: how many of the 104,236 `overlapping` queries repeat a pair was not counted.
