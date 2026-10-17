# LoopIdiomRecognize as a translation of LLVM's: the plan

TL;DR: `LoopIdiomRecognize.cpp` (3,881 lines at `d1106deb71cc`) is translated as `loopidiom/` and replaces `Fill` (the loop half of `fill.rs`). `fill` does the memset and memcpy idioms of the countable half and none of the noncountable half (popcount, ctlz/cttz, shift-until, strlen, CRC). Oversight's rulings: the slot is LLVM's (before `indvars` and full unroll); priority is low, behind LICM, InstCombine and LVI; the first PR moves `fill` to that slot and brings its cost (17.1 M of `d_faces` -O2) to LoopIdiomRecognize's on the same inputs; idiom-bearing inputs (L0) come before any further porting. `fill` acts on 0 of the 112 runs the replay corpus holds, so the corpus cannot judge it.

Rule for the code: each Rust function is a translation of the C++ function named in its first comment (permalink), the `cl::opt` defaults stand, and anything not translated is a numbered DEVIATE with a reason and a number. None is taken without oversight's approval.

## 1. What `fill` is today

`Fill` (1,604 lines in `fill.rs` with `Merge`) rewrites one loop at a time, then looks again. The loop has one latch, a straight-line body and a trip count `induction::counted` proves. It takes:

| shape | result | LLVM counterpart |
|---|---|---|
| a store or memset of a loop-invariant, one-byte-repeated value, GEP off an invariant pointer, stride = element size | `llvm.memset` | `processLoopStridedStore` |
| the same value with bytes that do not repeat | `llvm.experimental.memset.pattern` | the same, only with `memset_pattern16` or `-loop-idiom-force-memset-pattern-intrinsic` |
| several fills of bytes no other store reaches; adjacent same-byte fills tiled into one | one memset each, or one | none: LLVM takes each store alone |
| a load and a store of one cell each trip, same step, ranges apart | `llvm.memcpy` | `processLoopStoreOfLoopLoad` |
| the same with ranges that overlap, writes trailing the reads | `llvm.memmove` tagged `llrm.forward`/`llrm.backward` | the same function, `UseMemMove`, no tag |

Then `_pays` and `_cheaper_by` price a pattern, a copy or several fills against the loop (the target's string-instruction setup and per-cell cost; code bytes at -Os). A single constant-byte fill is rewritten always. The loop is kept for one trip and its counters leave with exit values (`counting::Seeds`); later passes delete it.

`Merge` (straight-line adjacent stores to a memset or wider stores) is MemCpyOpt's `tryMergingIntoMemset` and gcc's store merging, not LoopIdiom. It stays with `memcpyopt`.

Uses outside the pass: `loopdistribute.rs` (`fill::takes`, `filled_with`), `inline.rs` and `memcpyopt.rs` (`_copy`, `How`, `copy_price`).

Level and position: on from -O1 (the user's decision of 2026-10-10, [levels](../reference/levels.md): 8 of the 67 programs change at -O1, `scroll` 1,821,256 to 494,310 clocks), off at -O0 and -Og. Last in the function fixed point, after `unroll` and `peel`.

## 2. What LoopIdiomRecognize does

Run on each loop, innermost first, in LLVM's `LPM2` ([`LoopIdiomRecognizePass` before `IndVarSimplifyPass`, L562 at -O1](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L562) and [L737 at -O2/-O3](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L737)): after `LoopRotate`, `LICM` and unswitch, before `LoopDeletion` and `LoopFullUnroll`. The source says before (an earlier note said after). It is a loop pass, so it sees loops not yet fully unrolled.

| LLVM | does | llrm today |
|---|---|---|
| [`runOnLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L349) | preheader needed; gate by the target library (`has memset/memcpy`); countable loops to `runOnCountableLoop`, the rest to `runOnNoncountableLoop` | `Fill::run` |
| [`runOnCountableLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L383), [`runOnLoopBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L629), [`collectStores`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L591) | blocks of the loop not in a subloop; no loop that may throw; the stores that execute every trip | `fill::_fill` |
| [`processLoopStores`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L665), [`processLoopMemIntrinsic`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L810), [`processLoopMemSet`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L897), [`processLoopMemCpy`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L834) | store and memset/memcpy calls with a strided address | `_fill`, `_stored`, `_copied` |
| [`processLoopStridedStore`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1094), [`getMemSetPatternValue`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L440), [`mayLoopAccessLocation`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1017), [`getStartForNegStride`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1064), [`getNumBytes`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1082) | memset and memset.pattern from a stride, the loop's other accesses checked by alias analysis | `_memset`, `_pattern`, `_overlap`, `_stepped` |
| [`processLoopStoreOfLoopLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1350), [`avoidLIRForMultiBlockLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1582) | memcpy or memmove from a load and a store; no idiom in a multi-block loop at -Os | `_copied` |
| [`optimizeCRCLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1596) with `optimizeCRCLoopUsingClmul` (L1725) and `...UsingTableLookup` (L1852) | a CRC loop found by `HashRecognize`, as carry-less multiply or a Sarwate table | none |
| [`runOnNoncountableLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2002) | tries, in order: popcount, ffs, shift-until-bit-test, shift-until-zero, shift-until-less-than, strlen | none |
| [`recognizePopcount`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2855), [`detectPopcountIdiom`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2477), [`transformLoopToPopcount`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L3043) | `x &= x - 1` counting loop to `ctpop`, only when `TTI.getPopcntSupport(32)` is `PSK_FastHardware` | none |
| [`recognizeAndInsertFFS`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2767), [`insertFFSIfProfitable`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2703), [`isProfitableToInsertFFS`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2683), [`detectShiftUntilZeroIdiom`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2611), [`transformLoopToCountable`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2951) | a `while (x >>= 1) ++n` loop gets a `ctlz`/`cttz` trip count, if the intrinsic costs no more than the loop header | none |
| [`recognizeShiftUntilLessThan`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2785), [`recognizeShiftUntilBitTest`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L3345), [`recognizeShiftUntilZero`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L3727) | the same for shifts until a compare, a bit test or zero | none |
| [`recognizeAndInsertStrLen`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2219) | a byte (or wide) load-until-zero loop to a `strlen`/`wcslen` call, if the library has it | none |

`runOnLoop` leaves a function named `memset`, `memcpy`, `strlen` or `wcslen` alone. No bcmp/memcmp idiom exists in this file (the header names it a future one).

## 3. What MIR and the back end have

| need | MIR today | back end today |
|---|---|---|
| memset, memset.pattern, memcpy, memmove | `Intrinsic::MemSet`, `MemSetPattern`, `MemCpy`, `MemMove`, `llrm.forward`/`llrm.backward` | isel: moves up to 8, else `rep stos`/`rep movs` |
| ctlz, cttz, ctpop | none | none: no `bsf`/`bsr` is selected (`bswap` exists only as a table entry); `popcnt` does not exist before Nehalem |
| strlen, wcslen call | a libcall by name; `libfunc.rs` types it (`reads_arguments`) | the program's library; whether it holds one is the target's fact |
| trip count, stride, exit values | `induction::counted`, `derived`, `counting::Seeds` (no SCEV) | none |
| alias and memory facts | `Accesses`, `MemRef`, the MemorySSA being translated ([memoryssa-port](memoryssa-port.md)) | none |

So the memset/memcpy half ports onto what exists. The noncountable half needs `ctlz`/`cttz` (and `ctpop` if any target takes it) in MIR (`Intrinsic`, parser, printer, verifier, interpreter, `lint`, constant folding in `fold`), and selection: `bsr`/`bsf` exist on the 486 and Pentium, undefined on zero input, so the zero-input flag of the intrinsic is honoured as LLVM's `isProfitableToInsertFFS` prices it.

## 4. Translation map

| LLVM | Rust (`loopidiom/`) | DEVIATE |
|---|---|---|
| `runOnLoop`, `runOnCountableLoop`, `runOnLoopBlock`, `collectStores`, `processLoopStores`, `processLoopMemIntrinsic`, `processLoopMemSet`, `processLoopMemCpy`, `processLoopStridedStore`, `processLoopStoreOfLoopLoad`, `avoidLIRForMultiBlockLoop` | `loopidiom::run_on_loop` and one function each | D1 to D5 |
| `mayLoopAccessLocation`, `getStartForNegStride`, `getNumBytes`, `getMemSetPatternValue`, `isSameByteValueStore`, `getStoreStride` | the same names | alias queries on the translated MemorySSA; `induction` in place of `SCEV` until SCEV exists (D6) |
| `runOnNoncountableLoop` and the six recognizers with their `detect*` and `transform*` | the same names | target hooks (D7); intrinsics first (section 3) |
| `StrlenVerifier`, `recognizeAndInsertStrLen` | the same | emits a call only where the program's library holds it, as LLVM |
| `optimizeCRCLoop*`, `HashRecognize` | none | D9 |

DEVIATEs. A deviation is kept only where it rests on something MIR or the target lacks, with that fact. Status: approved (fact), dropped, pending.

- D1. `memset.pattern` whenever `fill` finds a pattern. LLVM: only with `memset_pattern16` or the force option; the target has no such libcall. Dropped; rows at S1 say what it cost, and it returns only with them.
- D2. Tiling of adjacent same-byte fills (`fill.rs` `_tiled`; `lru` -O2 150 to 133 clocks). Dropped from the idiom pass: LLVM makes a memset per store. `memcpyopt`'s port owns merging adjacent memsets; `lru` is the row to watch.
- D3. `llrm.forward`/`llrm.backward` on memmove. Approved: MIR's `memmove` carries no direction, and `rep movs` needs one (fact: the target's string move runs one way by the direction flag).
- D4. Pricing (`_pays`, `_cheaper_by`) against the loop. Pending a number: LLVM has none beyond TTI. The number is the target's `rep stos`/`rep movs` setup and per-cell clocks (`timings.times`) against the loop's; if S1 cannot name it per target, the pricing goes and only the single constant-byte fill stays.
- D5. The loop cut to one trip with `counting::Seeds` exit values, where LLVM zaps the stores and leaves the empty loop to `LoopDeletion` and `IndVarSimplify`. Approved until `LoopDeletion` is translated (fact: no loop deletion in the pipeline); then mirrored.
- D6. No `ScalarEvolution`: strides, counts and exit values come from `induction`/`counting`. Not a deviation to live with: a project-level dependency, sized separately.
- D7. `getPopcntSupport` and the FFS cost as target hooks. Approved: x86 has no fast popcount before Nehalem (fact: `PSK_FastHardware` never holds on 486/P5). `ctlz`/`cttz` need MIR intrinsics and `bsf`/`bsr` selection, which i386 and later have: a later representation item, not never.
- D9. CRC loops (`HashRecognize`, `Clmul`, table): not translated. Pending: no carry-less multiply on any target here; the table strategy is open.

(`isLibFuncEmittable` is LLVM's own rule, not a deviation: no `strlen` in the program's library, no call.)

## 5. Stages and gates

Priority: low. No time is taken from LICM, InstCombine or LVI.

| stage | content | gate |
|---|---|---|
| S1 | the first PR: `Fill` moves to LLVM's slot (before `indvars` and full unroll, mirroring the pipeline) and its cost on the same inputs is brought to LoopIdiomRecognize's, per IR instruction. D4 gets its number or goes | rows (section 7); `d_faces` -O1/-O2/-Os cycles of the pass against LLVM's |
| L0 | idiom-bearing inputs for the replay corpus, `difftest` gains `fill -> loop(loop-idiom)`; before any further port | the oracle reports a nonzero number; the 112 runs' zero is explained, not accepted |
| L1 | decisions only, countable half, against `opt -passes='loop(loop-idiom)'` and against `Fill` | counts per function; every difference classified |
| L2 | apply as `loopidiom/`, `Fill`'s loop half removed; `fill::takes`, `_copy`, `How`, `copy_price` move to their users | rows; compile cycles |
| L3 | (later) MIR `ctlz`/`cttz`, interpreter, fold, `bsf`/`bsr` selection | a unit test per layer; interpreter and emulator agree |
| L4 | noncountable recognizers | oracle counts; rows |
| L5 | `strlen`/`wcslen` | measured gap first: #1491 found none |
| L6 | delete `Fill`; `Merge` stays with `memcpyopt` | no use of the loop half left; docs regenerated |

`fill` is deleted by the PR that moves its last use (the loop half at L2: `loopdistribute`, `inline` and `memcpyopt` are re-pointed there).

## 6. Dependencies

- Position: `pass-list-parity` and `llvm-order-poc` own the loop slot; S1 follows their position.
- SCEV (D6): sized separately by the coordinator.
- ValueTracking (`isKnownNonZero`, `isDereferenceableAndAlignedPointer`): `valuetracking-port`; until it lands the FFS zero-input check is `ranger`'s fact.
- `LoopDeletion` (D5).
- MemorySSA ([memoryssa-port](memoryssa-port.md)) for `mayLoopAccessLocation`.

## 7. Rows

For every stage that changes output: the number of rows worse than base for each of clocks, executed instructions, memory operands and code bytes at each level (-O1, -O2, -O3, -Os) on m16 and m32, then the geomean, then the worst row. Clang is the reference; gcc only where it measures better. No waived rows.

## 7a. L0 inputs

Measured with the pinned `clang` 24.0.0git (`d1106deb71cc`, `~/scratch/llvm-build`), `-m32 -march=i486`, the 71 vsgcc programs, loop-idiom on against `-mllvm -disable-loop-idiom-all`, the calls it adds to the -O1 and -O2 IR:

| program | adds (-O1; -O2 where it differs) |
|---|---|
| `bintree`, `fpbench`, `histo`, `nbody_fixed`, `nbody_single`, `sieve`, `x_hash` | memset 2 |
| `lru` | memset 4 (-O2: 8) |
| `scroll` | memmove 4, memcpy 2 |
| `x_mergesort` | memcpy 3 |
| `x_radix` | memset 2, memcpy 2 (-O2: memset 6, memcpy 3) |
| `x_transpose` | memcpy 2 |
| `x_strlen` | strlen 2 |

No program adds `ctpop`, `ctlz` or `cttz` at `-march=i486` (`x_popcount` and `x_isqrt` hold the shapes; popcount needs fast hardware, so L4 also runs the pinned `opt` for a target that has it as an oracle-only check). The L0 corpus is these programs' pass inputs at -O1, -O2 and -Os, dumped as `tools/mkinputs.sh` does, beside the `fill_tests.rs` cases.

## 8. Oracle

Primary: the same IR into both. `tools/difftest` replays `opt -passes='loop(loop-idiom)'` of the pinned build (`~/scratch/llvm-build`, `d1106deb71cc`) on llrm's pass-input dumps, with the `LLRM_CHECK_*`/`LLRM_EXP_*` guard on, against the translated pass on the same module. Per function: the memset, memcpy, memmove and (later) ctlz/cttz/ctpop/strlen made, and the module where the two agree.

First measurement, 112 `fill` runs of the replay corpus (all configs): LLVM's loop-idiom adds no memory intrinsic, ctpop, ctlz, cttz or strlen call, and `fill`'s answer adds none. A synthetic `for (i < n) a[i] = 0` makes LLVM emit `llvm.memset`, so the instrument works; the corpus has no such loop in a function it dumps. That also puts `fill`'s 17.1 M cycles on `d_faces` -O2 at no use there; where it earns its keep is the bench programs of section 5, L0.

Second check: clang's IR for the bench C sources, `clang -O2 -Xclang -disable-llvm-passes -emit-llvm`, then `opt` to the same point and `loop(loop-idiom)`, against llrm's MIR at that point; per loop, classified, weaker evidence.
