# m68k float_in_memory_loop (2026-11-11)

TL;DR: 3,966,440 -> 1,774 clocks (68000 -O2; clang 1,798); 68020 1,581,984 -> 845 (clang 834).

First IR difference: the inner loop (16 trips, size 4) is gone in `pin opt` and kept in ours.
The pin unrolls it at [`shouldFullUnroll`, `UnrolledSize < UP.Threshold`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopUnrollPass.cpp#L921)
(64 < 150), with no analysis; `fadd` of constants then folds, and the outer loop folds with it.
m68k's `unroll_threshold_overrides` is 32, so ours falls to the analysis at
[L937](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopUnrollPass.cpp#L937),
which stops at `MaxIterationsCountToAnalyze` = 10 trips.
`LLRM_DEBUG=unroll`: base `no viable strategy found (trips 16, size 4, threshold 32)`;
with the cap lifted `analysis: unrolled 0 rolled 64`, then `unrolling by 16`.

Fix: `should_full_unroll` analyses every trip count of a loop the pin's own bar (150, 300 at -O3, 0 at -Os) would have unrolled outright.
Identity for a target that keeps the bar. DEVIATE UNROLL-ANALYZE, owner m68k float_in_memory_loop, exit 2026-11-20. `machine.rs` untouched.

## A/B (release/next 7ca36f1e02 vs this commit, tests/run/c, 68000+68020, -O2/-O3/-Os)

md5: base llrm-cc 4a889abc, llrm-llc 593c6eeb, llrm-m68k-llc d902b219; head llrm-cc 32f9e8e8, llrm-llc 0305f632, llrm-m68k-llc dfa12180 (wasm/arm64/6502/z80 llcs in dist md5.txt, kept in ~/scratch/fml).
834 cells compiled on both sides; 792 objects identical (all -Os); 42 differ (21 programs, -O2/-O3). All 84 changed exes run under qemu with the reference stdout.
A first A/B had a base worktree one merge ahead of the parent and showed 3 false -Os differences; redone.

Clocks (cycles --exe 5110bd14; 68000 base -> head; 68020 similar) and object bytes (68000):

| program | level | clocks | bytes |
|---|---|---|---|
| float_in_memory_loop | O2/O3 | 3,966,440 -> 1,774 | 904 -> 680 |
| fill_nest | O3 | 318,628 -> 103,540 | 872 -> 1216 |
| long_double_constant_store | O3 | 8,864 -> 6,248 | 688 -> 1272 |
| rep_count | O2/O3 | 85,460 -> 51,988 | 872 -> 968 |
| shrink_wrapped_frame | O2/O3 | 9,812 -> 4,570 | 788 -> 672 |
| loop_versioning | O3 | 592,166 -> 590,838 | 1252 -> 2400 |
| huffman_decode | O2/O3 | 136,142 -> 137,364 (+0.9%) | 1672 -> 1716 |

## Other targets (same two dists, tests/run/c, -O2/-O3/-Os)

x86_16-wat 534, x86_32-wat 522, x86_32-linux 426, x86_64-linux 417, arm64 417, wasm32 414, z80 348 cells: all compiled, all objects identical.
6502-sim65 351 cells: 348 compiled on both sides (3 refused on both), 350 identical; one differs, memmove_overlap -O3 (the shared analysis; the 6502 bar is `FULL_UNROLL_UNITS`).
Checked on sim65 (`tools/6502/ours_cr.py`, 6502, each side's llrm-cc/llrm-llc): PASS against the reference at O1, O2, O3, Os on both sides.
-O3 linked bytes 1076 -> 1296 (+220), clocks 17,484 -> 12,375 (-29%); O1/O2/Os identical (852/17,349; 1076/17,484; 730/17,569). The object-file size (16056 -> 6334) first quoted is not code bytes.

## The two worse rows

Pin opt (m68k triple, same C, `default<O3>`): loop_versioning "completely unrolled loop with 40 iterations" twice; huffman_decode -O2 unrolls a 6- and a 16-trip loop.
Ours now unrolls the same 40-trip loops (size 7 and 8, unrolled 280 and 320; the pin's -O3 bar is 300). The pin unrolls them at its own bar; the m68k override (32) was suppressing that.
m68k `unroll_milliclocks_per_byte` is 0 (unpriced), so a price veto needs a judged O2/O3 rate. Ruling asked of oversight.
The outer loop of sum_of_steps stays a loop; the pin closes it through IndVarSimplify exit values (main folds, so not executed here).

## Tried and dropped: an m68k unroll byte price

`unroll_milliclocks_per_byte = timing::byte_milliclocks(cpu)` (2000 mclk/B at 68000, 750 at 68020), A/B on the same 834 cells, 68000 clocks:
fixes loop_versioning -O3 at 68000 (-1148 B) and huffman bytes (-16 B, clocks +1.1%); 68020 loop_versioning/huffman unchanged.
Hurts logical_value -O3 3,112 -> 4,182 clk (+34%, +68 B; 68020 +9%), extension_chain_fused +2.4% clk, int64_division +8 B.
Cause (logical_value -O3, `LLRM_DEBUG=unroll`): loop %1, 3 trips, size 15, "saves 17 clk, adds 54 B (rate 2000)" is vetoed; base unrolls it and the whole nest folds to constants (`main` 7 instructions, against 20 with a loop). The estimate counts the loop's own folds (30 of 45 units live), not what the cleanup folds after the unroll.
The pin has no price veto, so it is only as good as its estimate. No rate is set; the 25 cells are not a held-out set. A nest-aware estimate would be a separate study.
