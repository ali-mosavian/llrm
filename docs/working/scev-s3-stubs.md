# SCEV stubs (S3), ranked

| Rank | Stub | Site | Measured effect |
|---|---|---|---|
| 1 | `isKnownViaInduction` (done here) | `scev/predicate.rs` | unroll-and-jam test6/test10 now fold `niter.next != 4` as the pin does (D5 deleted) |
| 2 | `getRangeForUnknownRecurrence` | `scev/range.rs` | translated (alim/feat/scev-s3-stubs); fail-first test; objects: pair s3b |
| 3 | `getRangeViaFactoring` | `scev/range.rs` | translated; fail-first test (0..39 -> 10..26) |
| 4 | `SolveQuadratic` (D-T2) | `scev/quadratic.rs`, `how_many.rs` | translated (DEVIATE Q1: coefficients up to 41 bits); ten quadratic loops equal the pin's counts; 0 of 71 rows programs reach it |
| 5 | `createAddRecFromPHIWithCasts` | `scev/create_addrec.rs` | DEVIATE 1 (no SCEV predicates): its result is a predicated recurrence. Exit item: the SCEV predicates row (`AllowPredicates`, `SCEVPredicate`s and their runtime checks); owner w-5; date 2026-11-27 for the plan |

Found while testing rank 1: `getStrengthenedNoWrapFlagsFromBinOp` passed no context instruction on a wrong reading of
`UseContextForNoWrapFlagInference`; the pin's `cl::init(true)`
([ScalarEvolution.cpp#L246](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L246)) uses the instruction as context.
