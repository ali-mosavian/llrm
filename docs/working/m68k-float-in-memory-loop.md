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

md5: base llrm-cc 4a889abc, llrm-llc 593c6eeb, llrm-m68k-llc d902b219; head llrm-cc 7af42b1c, llrm-llc 2ba30998, llrm-m68k-llc c5b95cfb.
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

Open: huffman_decode is slower and loop_versioning +1148 B for -0.2% clocks (both unrolled by the analysis's folded cost, as the pin would at its own bar).
The outer loop of sum_of_steps stays a loop; the pin closes it through IndVarSimplify exit values (main folds, so not executed here).
