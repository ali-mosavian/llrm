# softfloat

C soft-float runtime for targets without an FPU. RTLIB names (`__addsf3`, ...) over bit-level entry points (`sf32_add`, ...).

- `include/sf_helpers.h`: width helpers (clz, sticky shifts, 32x32->64); a target overrides one by defining its macro first.
- `src/sf32.c`, `src/sf32_conv.c`: binary32 arithmetic, compare, integer and binary64 conversions. Exact IEEE-754 round to nearest even, denormals, inf, NaN. Out-of-range float to int saturates, NaN gives 0.
- `src/rtlib.c`: the RTLIB wrappers.
- `tests/host_f32.c`: bit-exact oracle against the host FPU (NaN compared as a class):

```
cc -O2 -Iinclude tests/host_f32.c src/sf32.c src/sf32_conv.c -o host_f32 && ./host_f32 [n]
```
