# SCEV stubs (S3), ranked

| Rank | Stub | Site | Measured effect |
|---|---|---|---|
| 1 | `isKnownViaInduction` (done here) | `scev/predicate.rs` | unroll-and-jam test6/test10 now fold `niter.next != 4` as the pin does (D5 deleted) |
| 2 | `getRangeForUnknownRecurrence` | `scev/range.rs` | not measured |
| 3 | `getRangeViaFactoring` | `scev/range.rs` | not measured |
| 4 | `SolveQuadratic` (D-T2) | `scev/how_many.rs` | not measured |
| 5 | `createAddRecFromPHIWithCasts` | `scev/create_addrec.rs` | not measured |

Found while testing rank 1: `getStrengthenedNoWrapFlagsFromBinOp` passed no context instruction on a wrong reading of
`UseContextForNoWrapFlagInference`; the pin's `cl::init(true)`
([ScalarEvolution.cpp#L246](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L246)) uses the instruction as context.
