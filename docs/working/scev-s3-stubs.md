# SCEV stubs (S3), ranked

| Rank | Stub | Site | Measured effect |
|---|---|---|---|
| 1 | `isKnownViaInduction` (done here) | `scev/predicate.rs` | unroll-and-jam test6/test10 now fold `niter.next != 4` as the pin does (D5 deleted) |
| 2 | `getRangeForUnknownRecurrence` | `scev/range.rs` | translated (alim/feat/scev-s3-stubs); fail-first test; objects: pair s3b |
| 3 | `getRangeViaFactoring` | `scev/range.rs` | translated; fail-first test (0..39 -> 10..26) |
| 4 | `SolveQuadratic` (D-T2) | `scev/how_many.rs` | 0 of 71 rows programs (-O2, -Os) reach a quadratic exit or range: deferred until one does |
| 5 | `createAddRecFromPHIWithCasts` | `scev/create_addrec.rs` | its result is a predicated recurrence; predicates are DEVIATE 1 (none), so it cannot change an answer: deferred with predicates |

Found while testing rank 1: `getStrengthenedNoWrapFlagsFromBinOp` passed no context instruction on a wrong reading of
`UseContextForNoWrapFlagInference`; the pin's `cl::init(true)`
([ScalarEvolution.cpp#L246](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L246)) uses the instruction as context.
