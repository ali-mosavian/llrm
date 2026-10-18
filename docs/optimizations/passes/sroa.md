> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/sroa.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/sroa.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/sroa.rs sha256:d3b1ba0e46c24ef5235b3d3cde4e7bf2c5a6598611e34687f5e2f35d858bb907 -->

# sroa

`sroa-real`: scalar replacement of aggregates as LLVM's SROA does it: the
uses of an alloca are cut into slices, the slices into partitions that no
access straddles, each partition becomes an alloca of its own (or the old
one), the accesses are rewritten onto it, and what is then only loaded and
stored whole is promoted to registers. A translation of `SROA.cpp` at
d1106deb71cc (loads, stores, aggregate loads and stores, memset, memcpy and
lifetime intrinsics, phis and selects of pointers, pre-splitting).

DEVIATE, each with its reason (line numbers are SROA.cpp's):
- The mode is `sroa<preserve-cfg>` (`PreserveCFG`, L174): a select whose
  load is not speculatable, a store through a select, and a store through a
  phi on a critical edge are not rewritten (they need
  `SplitBlockAndInsertIfThenElse` or `SplitBlockPredecessors`), and the
  alloca is left. LLVM's early and late `sroa` of -O2 run with CFG
  modification allowed.
- Vectors (`isVectorPromotionViable` L2374, `rewriteVectorized*`
  L3451-L3637, `rewriteTreeStructuredMerge` L3002,
  `tryCanonicalizeStructToVector` L5460): the targets have no vector
  registers, and `AggregateToVector` is off at -O2.
- `propagateStoredValuesToLoads` (L6200), reached when a pointer only
  escapes read-only (it needs `SSAUpdater` and `LoadAndStorePromoter`): such
  an alloca is left as it is.
- Calls and invokes escape a pointer whatever their attributes
  (`visitCallBase` L1402 looks at read-only, nocapture operands).
- Address-space casts abort; pointers in a space that is a selector and an
  offset are not converted to integers (`canConvertValue` L2057).
- Debug records (`migrateDebugInfo` L348, the migration in `splitAlloca`
  L6066): an alloca a `Declare` names is left alone.
- `unfoldGEPSelect`, `unfoldGEPPhi` (L4578, L4673): gep over select or phi.
- `isSafeToLoadUnconditionally` (L1655, L1840) is `dereferenceable` alone:
  an alloca or a sized argument, no alignment, no global, no "a load of it
  already dominates" proof. `mayWriteToMemory` (L1613) counts any call as a
  writer.
- `fixLoadStoreAlign` (L4180): MIR cannot rewrite a load's alignment in
  place, so the loads and stores through a rewritten phi or select keep
  theirs.
- A memset or memcpy keeps no alignment or alias metadata of its own
  (`setDestAlignment`, `setAAMetadata`); a memmove split to a memcpy keeps
  its callee (`CreateMemCpy`, L4020); a lifetime marker names its size, as
  MIR's does.
- `AssumptionCache`, `DomTreeUpdater`, statistics, names that follow
  `.sroa.` suffix rules.
- Offsets are `i128` and do not wrap at the index width; an offset below
  zero is outside the alloca.
- `IRBuilder` here makes what it is told and does not fold constants
  (`ConstantFolder`) except the splat of a constant byte; `fold` does that
  after.
- Metadata: an access keeps its metadata where the type and offset are
  unchanged, and loses it otherwise (`adjustForAccess`).
