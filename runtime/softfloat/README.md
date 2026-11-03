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

Integrator hooks, defaults column-neutral. `-DSF_MUL32X32(a,b,h,l)` supplies a widening 32x32 multiply (`h:l` pointers). On the 6502, `-DSF_MUL32X32_6502` calls the hand-written `src/6502/mul32x32.s` (link it; `tests/asm6502/run_mul32x32.sh` checks it). On the 6502, `-DSF_DIV64_6502` (hand asm: not compiler output, its rows are marked so; the C path stays the default and the oracle) runs the f64 quotient loop as the hand-written `src/6502/div64.s` (link it; `tests/asm6502/run_div64.sh` checks it, `tests/run_sim65_f64.sh` with `SF_EXTRA` and `SF_ASM` the whole divide): sim65 `-Os` mean per divide 43159 to 12863 clocks. `include/sf_6502.h` (`-include`; sets `SF_SHIFT_STEPS`, not at `-Os`) shifts the addition's sticky right shift in byte then bit steps, because the 6502 shift libcall loops one bit per iteration: f32 add through llrm-cc -O2 243984 -> 198690 cycles over 64 operands, +5 B; mul and div unchanged; at `-Os` (bytes strict) the steps cost 18 B, so the header keeps the library default there (`tests/asm6502/shift_steps_bytes.sh`). `-DSF_DIV_NEWTON` divides by a reciprocal (eight 32x32 products) instead of 28 restoring steps. For 400 f32 divs, x86 m16 `-O2`: 404k clocks default, 277k with both; with `SF_DIV_NEWTON` alone 563k. x86 m32 through llrm-cc: 280k default, 222k with both; gcc: 191k default, 223k. The x86 m16 float runtime takes `include/sf_x86_m16.h` (both, plus `SF_PACK_INLINE` unless `-Os`; it beats or ties compiler-rt per op, `tests/x86_m16_kernels.sh`); both are also set and for llrm-cc x86 m32 (long becomes unsigned int there); not for gcc builds, m68k, 6502 or z80. `tests/host_div_newton.sh` checks exactness.


`tests/woz_compare/` runs the Rankin/Woz 6502 float routines (read at run time from the file `WOZ_LISTING` names (`tests/woz_compare/run.py`), never copied into the repo) against ours on sim65: reachable bytes and cycles per call on the same 64 operand values; `LLRM_CC=... python3 tests/woz_compare/run.py [n]`. Woz is not IEEE (24-bit truncating mantissa, no denormals/inf/NaN, 16-bit ints), so it is a size/speed column, not a correctness oracle; its results are checked to 2^-21 against the exact value.

`src/fast32.c` (spike): the fast mode's candidate, a 4-byte float in the Woz layout (exponent byte, 24-bit two's complement mantissa), truncating, no denormals/inf/NaN, saturating, 32-bit operations only, written from the algorithm's description. Oracle `tests/host_fast32.c` (double arithmetic, 2^-21); `tests/woz_compare/run.py` prints its bytes and sim65 cycles next to Woz's and the IEEE ones.

`src/fast32_woz.c`: the Rankin/Wozniak routines (Dr. Dobb's Journal 1976, public domain) translated routine by routine into C, flags and zero-page registers included. `tests/woz_compare/xcheck.py` runs the original machine code (built from the listing at run time) and this file on the same ~13,000 operands and requires identical bytes (overflow-vector cases skipped).

## 6502 assembly spike (`src/6502/sf32.s`)

Hand-written `__addsf3`/`__subsf3`: IEEE binary32, RNE, denormals, inf, NaN. Args rc2..rc5 / rc6..rc9, result A:X:rc2:rc3, scratch rc2..rc7 and rc16..rc25 only.
`tests/asm6502/run.sh [n]` checks it bit-exact against the host FPU on sim65 (4000 vectors: 0 fails; a broken tie rule gives 60 fails).
`src/6502/sf32_muldiv.s` adds `__mulsf3`/`__divsf3` (shift-add and restoring loops; links with `sf32.s` for rounding, packing and shifts; shared cells in `sf32.inc`).
`tests/asm6502/run.sh [n] [ops]` runs all four ops; `bench.sh` prints bytes and cycles per call on the Woz comparison operands; `first.sh op n` shows the first failing record.
Measured: add+sub 672 B, mul+div 609 B; cycles per call add 426, sub 435, mul 1615, div 2377. `src/6502/sf32_conv.s` adds the four int conversions (380 B; itof about 275 cycles, ftoi about 190 on the run.sh mix).
`src/6502/sf32_explog.s` adds `expf`/`logf` as byte-coded programs over the asm arithmetic (1042 B code, 83 B BSS, about 25k cycles per call), bit-exact against src/mathf.c; `tests/asm6502/runm.sh [n]` checks them on sim65.

## FP8 (`src/6502/sf8.s`)

IEEE-style 8-bit float `__fp8_add/sub/mul/div` (denormals, inf, NaN, round to nearest even), args rc2 and rc3, result A.
Build options (`ca65 -D`): `FP8_M` mantissa bits 1..4 (E6M1, E5M2 default, E4M3, E3M4; E + M = 7), `FP8_SPECIAL=0` drops inf/NaN, `FP8_DENORM=0` flushes denormals to zero, `FP8_ADD/SUB/MUL/DIV=0` leave a routine out.
`FP8_M=n FP8_SPECIAL=n FP8_DENORM=n tests/asm6502/run8.sh [ops]` checks all 65536 pairs against exact rational arithmetic on sim65 (all four widths and the flag combinations: 0 mismatches). `bench8.sh` prints bytes and mean cycles.
E5M2 sizes: 560 B everything; 438 B without inf/NaN; 541 B without denormals; 419 B without both; div alone 352 B. Cycles per call about add 393, sub 397, mul 337, div 380.

## 6502 size and speed table (sim65, 64 identical operands, call minus bare rts)

| op | Woz asm (not IEEE) B / cyc | generic C (llrm-cc -Os) B / cyc | IEEE asm B / cyc |
|---|---|---|---|
| add | 84 / 587 | 3109 / 3983 | 672 (add+sub) / 426 |
| sub | 191 / 775 | 3111 / 4014 | same code / 435 |
| mul | 196 / 3335 | 3409 / 13060 | 615 (mul+div, without the ~215 B shared with sf32.s) / 1615 |
| div | 163 / 3421 | 2911 / 7733 | same code / 2377 |
| i2f | 23 / 300 | 1735 / 2346 | 380 (four conversions) / ~275 |
| f2i | 51 / 913 | 611 / 1238 | same code / ~190 |

Not measured yet: m68k, m16/m32 and z80 against libgcc/compiler-rt/SDCC (needs the per-target runners and a free buildslot); the generic C columns come from the older pb3 build. No FP8 baseline exists in compiler-rt or libgcc.

## Table-driven FP8 multiply (`src/6502/sf8_mul_tab.s`)

`__fp8_mul_t`: one 16 KB lookup of the exact magnitude code (inf and NaN included, `tools/gen_fp8_mul_table.py` makes `fp8_mul_tab.inc`), sign by xor. 39 B of code, 46 cycles per call against 337 for the loop version (`sf8.s`), 0 mismatches on all 65536 pairs. The size is the table: it is only worth it where ROM is cheap. Add and sub cannot use the same trick (sign-aware, 64 KB).
`src/sf8.c` is the portable C version of the same algorithm (`tests/host_fp8.c`: exhaustive against double arithmetic on the host). `tests/asm6502/bench8c.sh` runs it compiled for the 6502.

## 6502 ABI (llrm-6502 release/next, rc2 first)

`src/6502/sf32_abi.s` holds the public `__addsf3/__subsf3/__mulsf3/__divsf3`: a in rc2..rc5, b's low half in rc6:rc7, its high half in the caller's stack words at (rc0:rc1)+0/+1, result A:X:rc2:rc3. The cores (`sf_add_c` ...) read b whole from rc6..rc9, so the shim copies the stack bytes into rc8/rc9 and restores the caller's values (they are callee-saved); `expf`/`logf` do the same. The driver checks that rc8/rc9 come back unchanged (leaving the restore out gives 105 fails of 300).
Cycles per call with the shim on the Woz operands: add 506, sub 515, mul 1730, div 2456 (shim about 80). 8-bit FP8 (rc2, rc3), `di3.s` and the conversions already match the layout. When the convention changes to A:X first (feat/6502-abi-ax), only `sf32_abi.s` and the 8-bit and `di3.s` entries change.

## m68k size vs compiler-rt (`tests/size_m68k.sh [CPU] [OPT]`)

Both compiled by llrm-cc (keep/llrm-cc_soft + llc_soft_merged, 2026-11-02), text bytes per object. 68000 -O2: `sf32.c` (add, sub, mul, div, cmp, neg) 3148 B, `sf32_conv.c` 1274 B. At -Os that build panics on `sf32.c` (liverange "Cannot overlap d"); `sf32_conv.c` is 836 B. compiler-rt's addsf3, mulsf3, divsf3 and floatsisf do not compile with the same build (`CtLzZeroPoison` has no libcall, `BitCast` in the older one), so there is no compiler-rt column on m68k yet; compiler-rt's fixsfsi is 162 B.

## binary64 (C)

`src/sf64.c`: `__adddf3 __subdf3 __muldf3 __divdf3 __negdf2`, the compares (`__cmpdf2 __eqdf2 __nedf2 __ltdf2 __ledf2 __gedf2 __gtdf2 __unorddf2`), `__floatsidf __floatunsidf __floatdidf __floatundidf` and `__fixdfsi __fixunsdfsi __fixdfdi __fixunsdfdi`; `__extendsfdf2` and `__truncdfsf2` are in `sf32_conv64.c`. The binary32 core at 56-bit working width.
Host oracle: `cc -O2 -fno-builtin -Iinclude '-DSF_NAME(n)=sfb_##n' tests/host_f64.c src/sf64.c src/sf32_conv.c && ./a.out [n]`; edges, random classes, rounding ties, int conversions. Each of: wrong product shift, tie rule, divide step count, sticky bit fails it (10^4..10^5 mismatches).

6502 build: `-include include/sf_6502.h` defines `SF_EXT_MULDIV`; `__mulsf3`/`__divsf3` then come from `src/sf32_6502.c` (link it beside `sf32.c`, `sf32_conv.c`). Reached -Os mul 3607 -> 2381 B, div 2911 -> 2343 B, sim65 cycles per call: mul -7%, div -5% at -Os; mul -21%, div -11% at -O2. Oracle: `tests/host_6502_muldiv.sh` (`CTL=1` must fail).
