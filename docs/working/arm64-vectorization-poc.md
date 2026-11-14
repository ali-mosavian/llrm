# arm64 vectorization POC: card (2026-11-10, branch alim/arm64-vec-poc, local, no merge)

TL;DR: the pin's LoopVectorize needs ~7 pieces; the first slice (memory-only loops, consecutive i8/i16/i32 load/store/splat) moves scroll by -1.08M instrs (+~150 B, +0.3·B = +45) and two small zero-fill cells. The other 15 vector cells need reductions, widening or FP lanes, one piece each.

Source: `matrix/arm64-judge-rejudge-pie-2026-11-09.tsv` (PIE neutral set, count.py --kernel), O2 net = instrs + 0.3·B. No O3 cells exist in it. External columns rebuilt for this card with `tools/arm64/build2.sh clangn|gccn` (pin clang d1106deb71, gcc 15.2).

## Correction to "29 of 33 are vector"
"ours ≤ best scalar column" is necessary, not sufficient. NEON opcode histograms of the clang/gcc objects:
- real vector loops: 18 of the 24 worse O2 cells (table below).
- not loop vectorization or negligible: nbody_fixed (+0.3 %), nbody_single (scalar, card A), queens, recgcd, recpow (scalar, cards C1/C2).
- ring: only gcc vectorizes (clang is 25 % worse than ours).

## Cells by the NEON feature they need (ΔI/ΔB = bar column minus ours; ΔB is an upper bound, the slice emits fewer epilogues)
| group | cell | ΔI | ΔB | net |
|---|---|---|---|---|
| M memory only (q ld/st, dup/movi) | scroll | -1,084,641 | +148 | -1,084,597 |
| | sieve | -2,915 | +168 | -2,865 |
| | bintree | -2,589 | +52 | -2,573 |
| W widening add/reduce (saddw, sxtl, addp, uadalp) | huge | -270,984 | +404 | -270,863 |
| | particle | -130,450 | +728 | -130,232 |
| | recsum | -6,227 | +248 | -6,153 |
| | quicksort / ring / tile / recchop | -2.6k / -1.7k / -1.2k / -0.8k | +168 / +100 / -184 / +772 | |
| A mul-add, shift (smlal, mul, usra, ld1/st3) | jpeg_fixed | -1,087,406 | +5,968 | -1,085,616 |
| | stft_fixed / histo / matmul | -169k / -27.6k / -0.9k | +672 / +120 / 0 | |
| F fp lanes (fmla, fmul) | jpeg_single | -1,627,155 | +4,500 | -1,625,805 |
| | stft_single | -928,747 | +576 | -928,574 |
| | nbody | -1,388 | +76 | -1,365 |
| I interleave (st2, tbl) | textfill | -27,889 | +408 | -27,767 |

Os: all worse Os cells are group C (beyond the pin, no vector). Pin rule `getEpilogueLowering` (LoopVectorize.cpp L7076): at optsize no epilogue, and runtime checks off (L7745), so only loops whose trip count is a known multiple of VF·IC vectorize. The slice must keep every Os byte count identical (A/B asserts it).

## Pieces, in dependency order
| # | piece | state | size | moves |
|---|---|---|---|---|
| P0 | MIR vector type, extract/insert/shuffle, splat constants | done (#7658, #7628) | - | - |
| P1 | DAG `MVT` v16i8/v8i16/v4i32/v2i64 (+v4f32/v2f64), `ScalarizeVector` action, builder gate. SD13 retires for these | **done** for the four 128-bit int vectors, behind `LLRM_ARM64_VECTORS` (b040175b96); x86 stays refused | - | prerequisite |
| P2 | arm64 backend: q register class + 16-byte spills (regalloc), LDRQui/STRQui/LDPQi/STPQi, MOVI/DUP, ADDv patterns from the pin's .td (`features.toml` drops vector types today: regenerate with the slice's types only) | **done for load/store**: tables regenerated with `[types] live`, `ldr/str q` select and run under qemu. Open: q spills, ldp/stp q, MOVI/DUP/ADDv (NEON-gated: needs `isNeonAvailable` true, which un-prunes scalar NEON patterns, so A/B it) | ~0.6k left | prerequisite |
| P3 | `has_vector_mir()` true for arm64; turn mid-end `int_bits().expect` panics hit by vectors into refusals (VECT-LATE ruling 11-06) | **flag done**; panics not yet audited (needed once the vectorizer emits other ops) | ~100 lines | prerequisite |
| P4 | `AArch64TTIImpl` vector hooks: getMemoryOpCost, getArithmeticInstrCost, getMaxInterleaveFactor, getMinTripCount... (`tti.rs` has `vector_register_bits` = 128 only) | missing | ~0.4k | prerequisite |
| P5 | LoopVectorize slice: legality (LAA exists), planner (VF from widest type, IC), VPlan widening of load/store/splat/IV, trip-count check, middle block, scalar epilogue, optsize rules | missing; LoopVectorize.cpp 8330 + Legality 2048 + VPlan* ~17k pruned by feature | ~1.5k | **group M** |
| P6 | post-vectorize cleanup arms: shuffle-of-shuffle (reverse copy), simplifycfg fold of the min-iter check at constant trip count | pass exists, no vector arms | ~200 | reverse loop of scroll |
| P7 | per group: reductions + widening (W), mul-add/shift (A), fp lanes + SLP (F), interleave groups (I) | missing | ~0.5k each | W 7 cells, A 4, F 3, I 1 |

## Step 2 slice (smallest that moves a real cell)
Forward consecutive i16/i8 copy and fill loops (P1-P3 on hand IR, then P5 without reverse). Cells: scroll (copy loops 1 and 4, fill loop 2), sieve and bintree (zero fill). Reverse loop 3 stays scalar until P6 (dated DEVIATE: shipping it without the shuffle fold emits rev64+ext pairs).

Gates: fail-first test per piece; scalar corpora md5-identical (the vector VT is only created from a vector IR type); A/B on `ccn` for bytes and executed instructions.

## Progress (b040175b96)
- Fail-first: `tests/vector_copy.rs` was red on release/next ("@copy: a <8 x i16> value"), then "CannotSelect Store" until the tables were regenerated; now `ldr q16,[x1]` / `str q16,[x0]`. Generator test `a_live_type_overrides_a_dead_prefix`.
- Run check: a `<8 x i16>` volatile copy loop (`ldr q16,[x10,x11]` / `str q16,[x9,x11]`) copies 1920 elements correctly under qemu (bad=0). Non-volatile it becomes `memcpy` by loop-idiom, as the pin does.
- Identity: 72 objects (36 bench programs, O2 and Os, `ccn`) byte-identical to release/next fd74ab9c6b with the flag off.
- Not done: P4 (TTI), P5 (vectorizer), so no cell has moved yet. Next: P4+P5 for scroll's forward copy loops, then P6.

## Progress (73c6f7fd04): P4 and P5 slice
- Pieces: legality (a5fe7da02f), TTI hooks (241bcbd9fe), planner (6a2fc8a96d, 57b65c9542), pass `loopvectorize.rs` + slot `loop-vectorize@1354` (73c6f7fd04). Planner VF/IC equal the pin's remark on 20 rows; pass tests: copy 1920 i16 VF 8 IC 2, offset-80 VF 8 IC 1 (pin remarks).
- First moved cell (flag on, `ccn`, O2, qemu/unicorn `--kernel`, output checked): scroll 1,194,464 -> 478,062 instrs (-716,402), 180 -> 192 B (+12). Loops 1 and 4 are `ldp q`/`stp q`; loop 2 (fill, needs splat) and loop 3 (reverse) stay scalar.
- Flag off: the slot returns before any analysis.
- A/B by arm64-judge-gaps (e980963e09 vs release/next 0aeb4e10e0, 36 programs x O1/O2/O3/Os, ccn, 144 rows, 0 unmeasured; handoff/matrix/arm64-vec-ab-{base,off,on}-2026-11-11.tsv): flag off 144/144 byte-equal; flag on exactly 4 objects differ, all scroll: O1 180->192 B, 1,194,464->478,062 i; O2 same; O3 216->224 B, 1,182,917->466,514 i; Os 176->176 B, 1,298,461->524,961 i. Bytes worse only on scroll O1/O2/O3 (+12/+12/+8 B), no instruction axis worse, nothing else moved.

## Premise count (2026-11-11, tip e980963e09 + LLRM_DEBUG=loop-vectorize, flag on, ccn, 47 programs x O2/O3, 261 innermost-loop verdicts)
Instrument: `LLRM_DEBUG=loop-vectorize` prints one verdict per innermost loop. Vectorized: 4 loops (scroll, 2 at O2 and 2 at O3). Refused, all 47 programs: unidentified PHI 62, non-intrinsic call 45, cannot if-convert 43, reductions 40, instruction not widened 24, no canonical induction 15, stored value not a loaded one 8 (fill), uncountable 6, memory conflicts 6, loaded value not only stored 5, element type 2, not consecutive 1.
Pin on the same sources (pin clang -O2 -fno-slp-vectorize, -Rpass remarks, the 18 vector cells): 34 loops vectorized, 1 per cell in sieve, bintree, recsum, quicksort, tile, matmul; 2 in huge, particle, recchop, histo, textfill; 3 in jpeg_fixed, stft_fixed, stft_single; 4 in jpeg_single; 5 in scroll (4 + main's init); 0 in ring and nbody. Ours: 2 of them (scroll).
O2 verdicts of the 18 cells (ours): fill ("stored value not a loaded one"): sieve, bintree, jpeg_fixed, jpeg_single, histo (1 loop each); non-0-start or counting-down induction ("no canonical"): scroll 2 (fill, reverse), recsum, recchop, textfill, jpeg_fixed, jpeg_single (1 each); reductions: scroll, huge, particle, quicksort, ring, tile, histo, textfill (1 each); calls in the loop: jpeg_fixed 9, jpeg_single 10, stft 1-2 (in aggregate the pin also misses 14 of the 17 jpeg_fixed loops, 14 of 18 jpeg_single; not matched loop by loop).
Next piece by loop count: induction from any start + fill (splat) + reverse = ~12 loops across 9 cells (scroll, sieve, bintree, recsum, recchop, textfill, jpeg_fixed, jpeg_single, histo); reductions = 8 loops across 8 cells but need widening and horizontal add (group W) in the backend.

## Progress (2026-11-10): backend, flag on compiles everywhere
- Pieces (pin d1106deb71cc): `visitINSERT_VECTOR_ELT` ([DAGCombiner.cpp#L25349](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L25349), `combine_vector.rs`, DEVIATE VEC2), AArch64 `LowerBUILD_VECTOR`/`LowerVECTOR_SHUFFLE` splats ([L17152](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L17152), [L16124](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L16124), `dag/vector.rs`, DEVIATE VEC1), the NEON patterns from the pin's .td for v16i8/v8i16/v4i32/v2i64 (`isNeonAvailable` = the flag), and the forms they emit: `dup`, `movi`/`mvni` (all `AArch64_AM` modified-immediate types), `movi d`, `ldr b/h`, `fmov` into a lane, `ucvtf s,s`. SD13 retired for the four 128-bit integer vectors (rows VEC1, VEC2 in selectiondag-plan.md).
- Flag on, `ccn`, 47 bench programs x O1/O2/O3/Os: 188 compiled, 0 failed (was 24 panics/refusals). Flag off (default, the line is `LLRM_ARM64_VECTORS` unset = off in `has_vector_mir`): arm64 188/188 objects identical to c33abb33fe at O1/O2/O3/Os; x86_16-dos, wasm32, m68k-68000, 6502-sim65, z80-sdcc at O1/O3/Os: same 86/77/78/76/77, diff 0, refused 8/17/16/18/17 (both sides refuse the same files).
- Vectorized cells, flag on vs off, unicorn `--kernel`, output ok in all 16 (O1/O2/O3/Os; bytes, executed instrs): scroll O2 180->200 B, 1,194,464->467,361 (-727,103); scroll Os 176->180, 1,298,461->511,461; histo O2 156->172, 67,599->66,898; jpeg_fixed O2 5,184->5,276, 12,033,780->11,934,498; jpeg_single O2 4,856->4,876, 9,138,148->9,009,370. Worse: jpeg_single O3 +4 B, +192 instrs with no loop vectorized (the pin's scalar NEON patterns, e.g. `ldr b; ucvtf s,s`, apply with the flag on). Flag stays default off; not for ready until that cell and the epilogue are settled.
- sieve and bintree vectorize no loop (their fills are taken by loop-idiom first). Next, in order: epilogue / tail folding (`getEpilogueLowering` L7076, `requiresScalarEpilogue`), reverse (down-counting induction, `REV64`), then reductions. Due 2026-11-12 18:00Z.

## Progress (2026-11-11): scalar remainder
- Pass: LoopVectorize `processLoop` ([L7714](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Vectorize/LoopVectorize.cpp#L7714)) with `getEpilogueLowering` (L7076) and the tiny-trip rule (L7809). Translation: `createVectorLoopSkeleton` (min.iters.check, vector.ph with n.mod.vf / n.vec, middle.block cmp.n, scalar.ph with bc.resume.val, the original loop as the remainder, `llvm.loop.isvectorized` + `unroll.runtime.disable` on both): `loopvectorizeskeleton.rs`. General: any single-block loop with an `eq`/`ne` exit on a stepping-by-one induction, constant or run-time trip count, any start.
- Planner: the pin's choice for a trip count VF x IC does not divide or an unknown one (20 pin rows added and 8 pin "none" rows asserted: i8..i64, trips 0, 3, 15, 16, 17, 33, 100, 1921), no epilogue at `-Os` or a constant trip under 16 (pin: none for x3 and x15). DEVIATE VEC3: the remainder is scalar; the pin vectorizes it too (`vec.epilog.*`).
- Tail folding: not on NEON. `AArch64TTIImpl::preferTailFoldingOverEpilogue` ([L7378](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L7378)) returns false without SVE and `getPreferredTailFoldingStyle` is `DataWithoutLaneMask`; the pin ends NEON loops in a scalar (or vectorized) epilogue, which is what this builds.
- Pin check of the padding fill: clang -O2 and -O3 report `jpeg_single.c:345:13: vectorized loop (vectorization width: 16, interleaved count: 2)`, our verdict.
- Backend: a 128-bit vector live across blocks (`vector.ph` splat used in the loop) stopped the compile ("a copy of a constant"); `CopyToReg`/`CopyFromReg` now take the vectors like floats. Fail-first test in `vector_copy.rs`.
- Correct: 9 kernels (non-divisible constant counts, run-time counts 0..80 from starts 0..5, an exit phi) checksum equal to the O0 reference under qemu at O1/O2/O3 (`b5658efffc9fc7cc`).
- Cells, flag on vs off, final binaries (llc md5 380924e90fdfa810f786a537d6ae8fdd, cc b948e11e18e97a81e197aa690fbea514), ok in all: scroll O2 +20 B, -727,103 instrs (positive control, unchanged); jpeg_fixed O2 +120 B, -128,882; O3 +120 B, -988; jpeg_single O2 +124 B, -143,759; O3 +136 B, +2,677. The remainder lets the row-padding fill (`for (k = w*3; k < stride; ++k) line[k] = 0`, at most 3 trips) vectorize, as the pin's does (VF 16, IC 2, both); it pays the check and gains nothing, as in the pin. Bar (pin clang, default vectorizer): jpeg_single O2 7,698,445 / O3 7,089,434 instrs, jpeg_fixed 11,123,095 / 10,623,344: the gap is the reductions and the widened arithmetic, next.
- Census (flag on, 47 programs x O2/O3, 261 verdicts): "scalar epilogue would remain" 2 (bintree), "trip count not a known constant" 6 (sieve, jpeg), now planned. The remainder is the prerequisite for the bench loops that follow (binary operations 24 loops, reductions 42), not a mover itself.

## Review fixes (2026-11-11, w15-arm64)

- LV-M1 (closed by slice 6): a loop LAA can only separate at run time (`RuntimePointerChecking::need`) is guarded by `vector.memcheck` (L1637), as the pin. Open: LV-M2, `isOutsideLoopWorkProfitable` (L7185) is not translated: the planner exposes no per-iteration costs and no TTI price of the check block, so a short loop may be widened where the pin's minimum trip count refuses it. Owner llrm-arm64-vec (planner), exit 2026-11-24. The SCEV-predicate checks (`vector.scevcheck`) stay out (LV-D5).
- LV-F1: no loop is widened at -O1 (`VectorizeOnlyWhenForced`); the earlier O1 scroll rows no longer apply. Exit 2026-11-24.
- IC is 1 wherever no epilogue is allowed (tiny trip count too), as `selectInterleaveCount` L3705.
- `LLRM_ARM64_VECTORS=0` is off.
- VP5/VP6/VP8 (planner header) state the register-pressure, low-trip and control-price deviations.

## LV-X2: integer binary operations (2026-11-11)

- **LV-OS (2026-11-11, oversight ruling): no vectorizing at -Os/-Oz** (`optsize`/`minsize`). The pin widens an optsize loop whose trip count VF divides; flag on that added bytes at Os (histo +4, scroll +4, jpeg_fixed +24, jpeg_single +16). Owner llrm-arm64-vec, exit 2026-11-24: the cause card, then the gate goes.

- Pass: LoopVectorize `VPWidenRecipe` (widen `add`, `sub`, `mul`, `and`, `or`, `xor`, `shl`, `lshr`, `ashr` over loaded and loop-invariant operands); NEON `addTypeForNEON`, `LowerVectorSRA_SRL_SHL`.
- Planner: the PIN rows already priced `add`; `mul <2 x i64>` is invalid (the pin leaves it scalar).
- Backend: `VectorBinary`, `VectorShift`, `VectorNot` (mc, text, encode checked against the pinned `llvm-mc`, lir round trip, regalloc wide registers); shifts by a constant splat lower to `VSHL`/`VLSHR`/`VASHR`.
- Fixed on the way (each with a fail-first test): the builder and legalizer truncated a vector shift amount to a scalar; predicate 13 (`1..16`) resolved to the SVE stub, so `ushr`/`sshr` of `<8 x i16>` never selected; instcombine and reassociate panicked on vector operations (DEVIATE COMBINE-VEC, REASSOC-VEC).
- DEVIATE LVP6/VEC4: a shift by a non-constant amount (`ushl`/`sshl`) is left scalar. Casts (`sext`, `zext`, `trunc`) need the type legalizer to split `<8 x i32>`: not translated, the loops with C integer promotion stay scalar. Exit LV-X4.
- Flag on vs off, same binary, unicorn ok (B, executed instructions): particle O2 272 -> 580 B, 154,349 -> 24,182 (clang 1000 B / 23,899; gcc 700 B / 28,495), O3 316 -> 516, 67,615 -> 24,266; sieve 172 -> 236, 17,927 -> 14,990; bintree 264 -> 288, 32,646 -> 29,599.
- Correct: 12 kernels (all lane widths, trips 0..90, in place and out of place) equal the gcc -O0 reference under qemu at O2 and O3.

## LV-X3a: scalar interleave, VF=1 IC>1 (2026-11-11, card)

- **Pass:** LoopVectorize `selectInterleaveCount` (L3705) at VF=1, `VPlanTransforms::unrollByUF` (`VPlanUnroll.cpp` L504) and the reduction recipes (`VPReductionPHIRecipe`, `bin.rdx` in the middle block).
- **Cells (pin `-Rpass`, 15 loops in 9 files):** sum reductions particle:32, scroll:20, quicksort:37, shellsort:20; init loops ring:11, histo:26. All `interleaved loop (interleaved count: 4)`; no vector type, no DAG work.
- **Shape (ring.ll):** body cloned IC times, part k's index `or disjoint %index, k`; `index.next = add %index, IC`; reductions get IC phis, folded in the middle block; the scalar remainder resumes with `bc.merge.rdx`.
- **Needs:** recipes with a price for every instruction kind (casts, geps, compares), not only load/store/binary; a replicate execute path; the skeleton's exit and resume phis for a live-out reduction (lifts LVS3 / LVP5).
- **Slices:** (1) integer add reduction over widened-style loads, IC copies; (2) other reduction kinds; (3) bodies with computed indices (ring). Exit 2026-11-20.
- **Measure:** executed and modelled clocks (`tools/arm64/clocks.py`); part of the win is instruction-level parallelism.

## LV-X4: casts and vector type legalization (2026-11-11, card)

- **Pass:** LoopVectorize `VPWidenCastRecipe`; `LegalizeVectorTypes.cpp` (`SplitVectorResult` L1375, `SplitVectorOperand` L3779, `SplitVecRes_ExtendOp` L3050, `SplitVecOp_TruncateHelper` L4841, `WidenVectorResult` L5223); AArch64 `addDRType` (L2752) / `addQRType`.
- **Why it is more than SplitVector:** `sext <8 x i16>` to `<8 x i64>` splits to four `<2 x i64>` results with `<2 x i16>` sources. In order: (1) 64-bit D-register vectors legal (v8i8, v4i16, v2i32, v1i64), (2) SplitVector, (3) WidenVector for what stays under 64 bits, (4) ext/trunc lowering (`sshll`, `ushll`, `xtn`). Skipping (1) means widening everything to 128 bits: output unlike the pin.
- **Cells:** histo:21 (`<16 x i16>`, trunc to `<16 x i8>`), particle init (`<8 x i64>`, trunc), recsum (`sext <2 x i32>`), scroll:28, plus the widened induction (`vec.ind`) they use.
- **Slices:** (1) D-register types and their selection; (2) SplitVector for load, store, binary, shift; (3) casts in the vectorizer; `vec.ind` (integer induction, plain or truncated) landed with WI 2026-11-13 (llrm-arm64-lv). No date; after LV-X3a.

## LV-W: widening plan for the counted cells (2026-11-12, tip b227f477a5, card; owner llrm-arm64-vec)

TL;DR: the pin's loop vectorizer removes the whole gap of six counted cells (`~/scratch/handoff/arm64-top-gap-cards-2026-11-12.md`). Our refusals say what each slice must lift; slices are in dependency order, each a draft until it moves a cell.

| cell (O2/O3 instrs gap) | pin loops (`-Rpass`) | ours today (`LLRM_DEBUG=loop-vectorize`) |
|---|---|---|
| jpeg_single +1.33M / +1.54M | :137 VF 4 (IDCT pass 1: 8 strided `i64` loads, `sitofp`, `fmuladd`), :145 VF 8 (pass 2), :345 VF 16 IC 2, :318 VF 2 IC 2 | :345, :318 done; IDCT loops "Found a non-intrinsic callsite" (`llvm.fmuladd`) |
| jpeg_fixed +0.95M / +1.13M | :165 VF 16, :345, :318; SLP horizontal reduction :150 | :345, :318 done; :165 "A binary result used other than by a store or a binary operation" |
| stft_single +0.99M / +5.49M (gcc) | :48 VF 4 IC 2, :52 VF 16 IC 2, :74 butterfly VF 4; gcc: :74 versioned | :52 "memory conflicts" (needs runtime checks), :48 "no vector of the element type" |

Slices (pin function in the PR body; each: fail-first, per-target md5 identity, compile-time wall clock):
1. **LV-D4 intrinsic calls** (`LoopVectorizationLegality::canVectorizeInstrs` call arm, `getVectorIntrinsicIDForCall`; `VPWidenIntrinsicRecipe`, `getIntrinsicInstrCost`): `fmuladd`, `fabs`, `smin`... Cells: jpeg_single IDCT (with 2, 3).
2. **FP lanes** (`VPWidenRecipe` fadd/fsub/fmul/fdiv/fneg, v4f32/v2f64 backend patterns): jpeg_single, stft_single.
3. **Casts and legalization**: LV-X4 slices (D-registers, `SplitVector`, `sitofp`/`fptosi`/`trunc`): jpeg_single pass 1 (`<4 x i64>` to `<4 x float>`), stft :48.
4. **Strided accesses** (`CM_Scalarize` replicate recipes, `VPReplicateRecipe`, the pin's `getMemInstScalarizationCost` already priced in #9473): jpeg_single pass 1 (`cosine[x][u]`), stft `tw[j*stride]`.
5. **Binary result used by other than store/binary** (select, compare, reduction users): jpeg_fixed :165, reductions (`VPReductionPHIRecipe`, horizontal add).
6. **Runtime alias checks** (`vector.memcheck`, `RuntimePointerChecking::generateChecks`, LV-M1): stft :52/:74, the O3 gcc bar. Own slice.
LV-OS gate stays (DEVIATE LV-OS, exit 2026-11-24 cause card). Bytes at O2 vs gcc: jpeg_single +980 B, jpeg_fixed +1,540 B (O2 rate cells: +0.3 net per byte, 294/462 instrs against gaps of 1.33M/0.95M); gcc does not unroll at O2, not a vector cause.

## LV-W2: what blocks the counted loops after slices 1-4 (2026-11-13, head 65d632f9a8; owner llrm-arm64-vec)

TL;DR: float lanes, `fmuladd`, same-width converts and scalarized strided accesses are in (#9538); no counted cell moved. The shared blocker is vector **type legalization** (`LegalizeVectorTypes.cpp`: `SplitVectorResult/Operand`, `WidenVector*`) and D-register types; `legalize_types.rs` has `TypeAction::ScalarizeVector` as a name only ("vector with no register class").

| loop (real `llrm-cc -O2` IR) | refusal now | needs |
|---|---|---|
| jpeg_single pass 1 (%520) | "A cast that changes the lane width": `coef` is `long`, `sitofp <4 x i64>` to `<4 x float>` | split `v4i64` (BUILD_VECTOR, INSERT, SINT_TO_FP), `v2f32` D-register, `fcvtn`; scalarized strided load/store done |
| jpeg_single pass 2 (%639), :701 | binary result / cast width | same, plus `fptosi`, `smin/smax` (llrm-target-arm64 owns LV-D4 vectorutils), byte stores (`trunc`) |
| jpeg_fixed convert (%795) | "Found a non-intrinsic callsite": `llvm.smax/smin.i32` | LV-D4 `smax/smin` recipes, `zext/trunc` `i8`<->`i32` (v8i8/v4i16 D types), stride-3 store (interleave group, factor 3), shift by an invariant amount (LVP6) |
| jpeg_fixed IDCT (%531) | "possible but not beneficial": `mul <2 x i64>` is unpriced (14 in the pin) | price the scalarized `mul`/`sdiv` of `v2i64` as the pin does |
| stft_single (%54, %93) | memory conflicts; strided `tw[j*stride]` runtime stride | runtime alias checks (own slice), gather-by-scalarize with a symbolic stride |

Order proposed: (1) vector type legalization in the DAG legalizer (translate `LegalizeVectorTypes.cpp` for the node set the loops use) with D-register types; (2) widening of `trunc/zext/sext` and width-changing converts; (3) `smax/smin` and the rest of the intrinsic list on llrm-target-arm64's `vectorutils`; (4) pin's `mul <2 x i64>` price; (5) runtime alias checks. Slices 1-4 stay drafts until (1)-(2) move jpeg_single.
