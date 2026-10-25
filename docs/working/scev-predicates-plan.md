# SCEV predicates (S3 rank 5, DEVIATE 1): plan (2026-10-23)

Owner w-5. Plan 2026-11-27 (this note); code dates below. Pin: [ScalarEvolution.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp) at d1106deb71cc.

Ours has none of it (DEVIATE 1 of docs/architecture/scev-port.md): no `SCEVPredicate`, no `AllowPredicates`, no `PredicatedScalarEvolution`. What it costs: a loop whose trip count the pin proves under a runtime check (an unproven wrap of a counter, a `zext`/`sext` of a narrower recurrence) is unknown to us. The corpus oracle (`tools/scev/counts.py`, 306 IRs) fails on exactly 3 files, all "predicated exit count"; that is the whole measured cost today.

## The pin's pieces, in file order

| Pin ([line](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp)) | What | Status |
|---|---|---|
| [`createAddRecFromPHIWithCasts`, L5432](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L5432) and `createAddRecFromPHIWithCastsImpl` | the recurrence `phi(trunc(x) ext)` with a wrap or equal predicate | MISSING |
| [`getPredicatedBackedgeTakenCount` and the symbolic/constant max forms, L8525](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8525) | the trip count with its predicates | MISSING |
| `computeExitLimit`, `howFarToZero`, `howManyLessThans`, `howManyGreaterThans` with `AllowPredicates` ([L10435](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L10435), [L10774](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L10774)) | the second try with predicates | DEVIATE 1 (gives up where it would add one) |
| [`getEqualPredicate`/`getComparePredicate`, L15200](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L15200); [`getWrapPredicate`, L15225](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L15225) | uniqued predicates | MISSING |
| [`SCEVPredicateRewriter`, L15244](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L15244); [`convertSCEVToAddRecWithPredicates`, L15372](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L15372) | rewrite an expression to an addrec under predicates | MISSING |
| [`SCEVComparePredicate`, `SCEVWrapPredicate`, `SCEVUnionPredicate`, L15413](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L15413) (`implies`, `isAlwaysTrue`, `add`) | the predicate classes | MISSING |
| [`PredicatedScalarEvolution`, L15598](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L15598) (`getAsAddRec`, `addPredicate`, `getPredicatedBackedgeTakenCount`) | a client's view that accumulates its predicates | MISSING |

## Consumers (what needs them)

Runtime-checked clients: LoopAccessAnalysis (runtime pointer checks, `PSE`), LoopVersioning, loop vectorization and distribution. The corpus has none of these passes translated that rely on a predicate being added (LAA translates its dependence part; its runtime checks stand on `PredicatedScalarEvolution`).

## Order

1. **P1 (landed in #4321, `scev/wrap_predicate.rs`; was code 12-04):** the predicate classes and uniquing (`SCEVPredicate`, compare, wrap, union), with `implies` / `isAlwaysTrue` unit tests against the header's contract; `ScalarEvolution::get*Predicate`.
2. **P2 (landed in the PR that adds `scev/predicate_rewrite.rs`; was 12-11):** `SCEVPredicateRewriter`, `convertSCEVToAddRecWithPredicates`, `createAddRecFromPHIWithCasts` (the 3 corpus files: oracle fail-first).
3. **P3 (landed in the PR that adds the predicated counts; was 12-18; the oracle's 3 files pass, 306 of 306):** `AllowPredicates` through `computeExitLimit`/`howFarToZero`/`howManyLessThans`; the predicated backedge-taken counts cache.
4. **P4 (landed in the PR that adds `scev/predicated.rs`; the LAA runtime checks that call it remain):** `PredicatedScalarEvolution`.

Each lands when it builds and passes the gate; the oracle's 3 files are the measure of P2/P3 (they must stop failing, no other file may start).
