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
