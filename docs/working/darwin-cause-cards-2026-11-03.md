# arm64-macos cause cards, llrm-cc 8c153e6f0 vs pin and gcc-13 (alim, due 11-06)

TL;DR: 72 cells (29 programs, 4 levels) are >10% worse than the best of pin and gcc-13 (bytes or µs). 15 have a static gap to the pin (translation, fixable, A). 6 vanish when vectors are off (C). The rest are pin-faithful or smaller than the pin and lose only to gcc-13 or to timing noise (B, for oversight). The sets can overlap.

Instrument: `tools/darwin_bench/novec.py` and `sameir.py`; pin and gcc-13 each with and without vectorization, kernel-only bytes (`codebytes.kernel`), same-IR run (pin `-fno-vectorize` IR through `llrm-arm64-llc`, bytes and insns). Counts and md5s are in the table headers (150/150 cells per level; pin clang c0ff2112, gcc-13 fb695479, llrm-cc d00fc3a5). µs is not used as evidence: the Mac ran desktop apps beside the bench (spreads 10-300% on re-timing); at O2 only fib, rectwo, recfloat, recsum, sieve and ring held their sign in interleaved re-timing. The pin builds with the default stack protector, llrm-cc has none (sieve Os: pin +7 insns); it flatters llrm-cc by that much on kernels with arrays.

## A. Gap to the pin (ours >5% over pin-nv), by size
Same-IR check says where: llc(pinIR) = our backend on the pin's mid-end.

| cell | ours / pin / gcc B | same-IR | cause (mnemonic diff; * = read in asm) |
|---|---|---|---|
| nbody_fixed O2 | 932 / 788 / 548 | llc(pinIR) 776: mid-end +156 B | spills: str 29/10, ldr 25/13, mov 49/32; shifts lsl 6/0, asr 4/0 (strength reduction / fixed-point mid-end) |
| nbody_fixed O3 | 1172 / 1056 / 1488 | llc 1116: mid-end +56 | same |
| particle O3 | 380 / 272 / 336 | llc 396: mid-end | unrolled 95 insns vs 68 (cause not read) |
| grep Os | 648 / 600 / 592 | llc 652: = ours | mov +13, add +9, cbnz 7/3 vs b.ne: copies not coalesced, branch shape |
| recpow O1 | 332 / 308 / 328 | llc 332 | backend: pin 77 insns vs 83 |
| scroll Os | 172 / 152 / 192 | llc 172 | mov 15/11: copies |
| recgcd O1-Os | +16 B each | llc = ours (backend) | tail-call loop: extra `mov`, `b` (C4/C7) |
| recmany Os | 192 / 180 / 132 | llc 192 | sub 5/3, lsl 1/0, subs/b.ne vs sub/cbnz |
| mandel Os | 148 / 136 / 128 | llc 148 | mov 12/9 |
| tile O1, Os | 80 / 72 / 72 | llc 80 | * two copies: `mov w12,#0xe` kept outside and copied (`mov x14,x12`) instead of rematerialized in the loop; IV `add w13,w10,#1` placed before `cmp w10` then `mov x10,x13` (pin: `cmp w8; add w8,w8,#1`, in place) |
| recfloat O1 | 100 / 92 / 120 | llc 72: mid-end +28 | * loop-carried FP value copied: `fmadd d19,..; fmov d16,d19; b` (+2 `fmov` at entry), back-edge `b` where the pin has rotated `cbnz` (C4); constants are hoisted, not in the loop |

General causes, each a mechanism rather than a cell:
1. Register copies not coalesced (`mov` +1 to +13 in grep, scroll, mandel, sieve, queens, tile, recgcd): coalescer / phi copies (greedy-subreg's remat fix addresses the immediate-move part only).
2. Loop-carried values copied through a new register (tile, recfloat *): a phi whose update is not coalesced with it; same family as 1.
3. Spills and missing ldp/stp pairing in FP/fixed-point kernels (C5): pin pairs spills (`stp` 0/10 in fpbench O2, `ldp` 1/35 in matmul).
4. Large-frame prologue (sieve Os *): ours `sub sp,#0x420; add x16,sp,#0x400; stp [x16]` (+4 insns, prologue and epilogue); the pin pushes callee-saves with pre-index `stp` first, then `sub sp`.
5. Conditional compare (sieve, queens Os *): pin `cmp; ccmp; b.hs`; ours two compares and two branches. The pin's AArch64ConditionalCompares has no counterpart.
6. Duplicated `mov x0,xzr; b ret` blocks (sieve Os *): tail merge / placement (C7; w15's edge-placement card found the pin pays the same `b` in `switch`, so only the genuinely duplicated returns count).

## B. Pin-faithful, gcc smaller or faster (bar vs gcc; oversight)
ours within 5% of the pin; gcc-13 smaller by >10%. Grouped by what gcc does:
- Recursion inlined by gcc (self-recursive inlining): fib O2/O3 (27 insns vs 304-333, ~2x faster), rectwo O2/O3 (24 vs 188, 3.7x faster), recchop (56 vs 408), frames (49 vs 252), recpow O3 (83 vs 408), queens O3 (75 vs 1004). Bytes go the other way (gcc is 7-12x bigger). Open CLOCK row, pin-gap (the pin has no self-recursion inlining): only a translated pass closes it, no row fix.
- Loops left rolled where the pin and we unroll: crc O2 (39 vs 25 insns), matmul O2 (390 vs 80), shellsort O2/O3 (140/321 vs 68/52), tile O3 (92 vs 20), sieve, ring. Bytes only; the pin unrolls these.
- Os: gcc-13 is smaller than the pin on recpow (324 vs 200), switch (320 vs 256), queens, recchop, recsum (72 vs 40), fib, frames, ring, recfloat, sieve: -Os of the pin is not the floor. Needs a priced size model (Os: bytes per call/prologue) or the gcc mechanism; not a row fix.
- FP: fpbench, nbody_single O2/O3 (212 vs 116, 213 vs 120): gcc has fewer insns (fmadd 3 vs 18 in fpbench O2; cause not read); pin-nv = ours.

## C. Vector-deferred (listed apart, not wins)
histo O2/O3 (µs vs gcc 2.1/1.6, vectors gone with -fno-vectorize), particle O2 (pin 1.9 vs 8.6), matmul Os, quicksort O3.

## Next
Verify A1-A6 each with a fail-first test on a minimal function before any fix; A4 and A5 are the largest unrewritten mechanisms (every function with a large frame / a bounds-check pair).

## D. DOS-flavoured programs (host shim)
huge, lru, textfill, jpeg_fixed, jpeg_single, stft_fixed, stft_single did not build on arm64 (far/huge, `<io.h>`, input files). `tools/hostshim/host.py` (shared; cc_check.sh and scoreboard.py can reuse it) builds them for all compilers: flat `-Dfar= -Dhuge= -D__huge=`, POSIX `os.c`, a host text-screen buffer, data files in the run dir; one-shot programs (jpeg_*) are timed one call per process. Test: `test_host.py`.
- llrm-cc fails to compile stft_fixed and stft_single: `llrm-arm64-llc: bitreverse of i8` and `Legalize(CtTzZeroPoison, I32)`; for w15 (unsupported intrinsics, not an instrument issue).
- stft_single prints a value that differs from the x86 `.out` on every compiler (its libm twiddles, and `-ffp-contract` changes it): its output is unverified; rows are bytes-only evidence.
- lru, textfill measure flat code, not real-mode far access.
