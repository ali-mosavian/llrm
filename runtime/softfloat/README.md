# softfloat

C soft-float runtime for targets without an FPU. Every entry point has its canonical compiler-rt/libgcc name (`__addsf3`, `__floatsisf`, `__fixunssfdi`, `__cmpsf2`, `__muldi3`, ...), implemented over static bit-level cores.

- `include/sf_helpers.h`: width helpers (clz, sticky shifts, 32x32->64); a target overrides one by defining its macro first.
- `src/sf32.c`, `src/sf32_conv.c`, `src/sf32_conv64.c`: binary32 arithmetic and compare; 32-bit integer conversions (no 64-bit type); 64-bit integer and binary64 conversions. Exact IEEE-754 round to nearest even, denormals, inf, NaN. Out-of-range float to int saturates, NaN gives 0.
- `tests/host_f32.c`: bit-exact oracle against the host FPU (NaN compared as a class):

```
cc -O2 -fno-builtin -Iinclude '-DSF_NAME(n)=sfb_##n' tests/host_f32.c src/sf32.c src/sf32_conv.c src/sf32_conv64.c -o host_f32 && ./host_f32 [n]
```

Vector classes: edge list (zeros, denormal limits, min/max normal, one, inf, NaN payloads, powers of two near 2^24..2^32, pi); random mix of uniform bits, denormals, near-equal exponents (cancellation), edge values plus or minus 3 ulp; every 251st f32 pattern for conversions; i32/u32/i64/u64 powers of two plus or minus 4, 0..70000 and its negation, random widths, and exact round-to-nearest ties and half-plus-minus-one neighbours at every bit position; f64 to f32 random with exponents spanning the denormal range and half-way ties; f32 to f64 to f32 round trip.

Integer builtins (`__mulsi3 __udivsi3 __divsi3 __umodsi3 __modsi3`, `__muldi3 __udivdi3 __divdi3 __umoddi3 __moddi3`): `src/int32.c`, `src/int64.c`, no 64-bit operation in any body. Division by zero gives all ones and remainder = dividend; INT_MIN / -1 is INT_MIN. Oracle `tests/host_int.c` (build line in its header comment: `-fno-builtin '-DSFB_NAME(n)=sfb_##n'`).

Math (`src/mathf.c`, coefficients in `include/sf_coef.h` from `tools/gen_coef.py`, stdlib Python): `sqrtf` (exact), `sinf cosf tanf atanf atan2f asinf acosf expf exp2f logf log2f log10f`, written in float arithmetic so they call the target's `__mulsf3` etc. Max error vs the host double libm over 24M inputs: sin/cos 3 ulp, tan 4, asin/acos 3, atan/atan2 2, exp/exp2/log 1, log2/log10 2 (the 3-4 only beyond |x| > 2^20, Payne-Hanek path; below 200 it is 2). Not yet: `powf` (needs f64 for the extra precision). Oracle `tests/host_mathf.c` (build line in its header). Compiles only on targets that soften float (m68k, once the SoftenFloat binary is current; 6502 has no float lowering yet).

`-DSF_INTABI` builds the canonical entry points with `uint32_t`/`uint64_t` in place of `float`/`double` (same bits, same soft-float calling convention) for backends with no float type yet (6502, z80 today). `mathf.c` has no such mode.

`tests/woz_compare/` runs the Rankin/Woz 6502 float routines (read at run time from `~/scratch/hand-sources/site/floats/wozfp3.txt`, never copied into the repo) against ours on sim65: reachable bytes and cycles per call on the same 64 operand values; `LLRM_CC=... python3 tests/woz_compare/run.py [n]`. Woz is not IEEE (24-bit truncating mantissa, no denormals/inf/NaN, 16-bit ints), so it is a size/speed column, not a correctness oracle; its results are checked to 2^-21 against the exact value.

`src/fast32.c` (spike): the fast mode's candidate, a 4-byte float in the Woz layout (exponent byte, 24-bit two's complement mantissa), truncating, no denormals/inf/NaN, saturating, 32-bit operations only, written from the algorithm's description. Oracle `tests/host_fast32.c` (double arithmetic, 2^-21); `tests/woz_compare/run.py` prints its bytes and sim65 cycles next to Woz's and the IEEE ones.

`src/fast32_woz.c`: the Rankin/Wozniak routines (Dr. Dobb's Journal 1976, public domain) translated routine by routine into C, flags and zero-page registers included. `tests/woz_compare/xcheck.py` runs the original machine code (built from the listing at run time) and this file on the same ~13,000 operands and requires identical bytes (overflow-vector cases skipped).

## 6502 assembly spike (`src/6502/sf32.s`)

Hand-written `__addsf3`/`__subsf3`: IEEE binary32, RNE, denormals, inf, NaN. Args rc2..rc5 / rc6..rc9, result A:X:rc2:rc3, scratch rc2..rc7 and rc16..rc25 only.
`tests/asm6502/run.sh [n]` checks it bit-exact against the host FPU on sim65 (4000 vectors: 0 fails; a broken tie rule gives 60 fails).
`src/6502/sf32_muldiv.s` adds `__mulsf3`/`__divsf3` (shift-add and restoring loops; links with `sf32.s` for rounding, packing and shifts; shared cells in `sf32.inc`).
`tests/asm6502/run.sh [n] [ops]` runs all four ops; `bench.sh` prints bytes and cycles per call on the Woz comparison operands; `first.sh op n` shows the first failing record.
Measured: add+sub 672 B, mul+div 609 B; cycles per call add 426, sub 435, mul 1615, div 2377. `src/6502/sf32_conv.s` adds the four int conversions (380 B; itof about 275 cycles, ftoi about 190 on the run.sh mix).
`src/6502/sf32_explog.s` adds `expf`/`logf` as byte-coded programs over the asm arithmetic (1042 B code, 83 B BSS, about 25k cycles per call), bit-exact against src/mathf.c; `tests/asm6502/runm.sh [n]` checks them on sim65.

## FP8 E5M2 (`src/6502/sf8.s`)

`__fp8_add/sub/mul/div` (OCP E5M2: denormals, inf, NaN, round to nearest even), args rc2 and rc3, result A. `tests/asm6502/run8.sh` checks all 65536 pairs of each op against exact rational arithmetic on sim65: 0 mismatches.
`tests/asm6502/bench8.sh` prints bytes and mean cycles over 4096 pairs. 566 B for the four ops (first version 680 B); cycles per call add 399, sub 403, mul 337, div 380.
