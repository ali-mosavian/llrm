# arm64-macos winners, llrm-cc a924b93b6 vs clang (pin) and gcc-13 (alim, 11-03)

TL;DR: ours has the fewest bytes in 15/14/20/7 cells (O1/O2/O3/Os, of 37) and is >10% above the best in 3/9/5/18; gcc-13 wins Os bytes in 25.
Source: `arm64-darwin-matrix-2026-11-03.tsv` (440 rows; 148 cells per compiler minus 4 stft cells ours cannot compile at O2/O3; median of 11, load and spread per row; clocks marked noisy are not used here). Cause cards: `darwin-cause-cards-2026-11-03.md` (A, B, C, D).

| level | bytes best: ours / clang / gcc13 | ours >10% above best (bytes) | ours >10% above best (clocks, quiet cells) |
|---|---|---|---|
| O1 | 15 / 14 / 8 | histo, jpeg_fixed, jpeg_single | fib, frames, mandel, ring, tile |
| O2 | 16 / 8 / 11 | crc, fpbench, jpeg_fixed, jpeg_single, lru, matmul, nbody_fixed, nbody_single, recgcd, shellsort | frames, huge, recgcd, ring, textfill |
| O3 | 20 / 10 / 5 | fpbench, nbody_single, recgcd, shellsort, tile | fib, frames, huge, quicksort, recpow, sieve |
| Os | 7 / 5 / 25 | fib, frames, huge, lru, mandel, queens, recchop, recfloat, recgcd, recmany, recpow, recsum, ring, scroll, sieve, stft_fixed, switch, tile | recfloat, recsum, ring, textfill |

Cause, by card: 
- A (gap to pin): nbody_fixed, recgcd, recmany, mandel, scroll, tile, recfloat, recpow(O1).
- B (gcc only): fib/frames/recpow/recchop/queens clocks and Os bytes (self-recursion inlining, open clock row); crc, matmul, shellsort, tile O3 (gcc/pin loops); fpbench, nbody_single (FP); Os bytes of recsum, ring, sieve, switch.
- Untraced (new with the 7 programs or new cells): jpeg_fixed/jpeg_single (O1-O2: 7-8 KB vs 4.2-4.5 KB gcc), huge clocks (25 vs 6-10 us), lru O2/Os, textfill clocks (1.8 vs 0.3 us), stft_fixed Os, histo O1, quicksort/sieve/recpow O3 clocks, frames/ring/mandel O1 clocks.
- Not comparable: stft_single prints a value that differs from the x86 `.out` on every compiler (clang = gcc13); ours cannot compile stft_fixed/stft_single at O2/O3 (bitreverse i8, cttz zero-poison; routed to w15). lru, huge, textfill are flat reading.
