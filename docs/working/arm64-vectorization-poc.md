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
