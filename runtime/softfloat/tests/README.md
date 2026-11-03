# Tests

Host oracle (run from `runtime/softfloat`, each prints `fails=0` or its totals):
- `cc -O2 -fno-builtin -Iinclude '-DSF_NAME(n)=sfb_##n' tests/host_f32.c src/sf32.c src/sf32_conv.c src/sf32_conv64.c -lm && ./a.out`
- `cc -O2 -fno-builtin -Iinclude '-DSF_NAME(n)=sfb_##n' tests/host_f64.c src/sf64.c && ./a.out`
- `cc -O2 -fno-builtin -Iinclude '-DSFB_NAME(n)=sfb_##n' tests/host_int.c src/int32.c src/int64.c && ./a.out`
- `cc -O2 -Iinclude tests/host_fast32.c src/fast32.c -lm`; `tests/host_mathf.c` and `tests/host_fp8.c`: the build line is in their header
- `tests/host_div_newton.sh` (SF_DIV_NEWTON, two multiplies); `tests/host_add_far.c` (far-exponent add/sub; build line in its header)
- `sh tests/int16_lint.sh` (16-bit int compile of every source)

Targets:
- m68k (qemu): `LLRM_CC_DIR=dir tests/run_m68k.sh CPU OPT [n]`, `tests/m68k_cmp_unordered.c`, `tests/m68k_wrapper_size.sh`, `tests/m68k_no_self_call.sh`, `tests/size_m68k.sh [CPU] [OPT]`
- z80 (ucsim): `tests/z80/run.sh [ops]`
- 6502 (sim65): `tests/asm6502/run.sh [n] [ops]`, `tests/asm6502/bench_wide.py`, `LLRM_CC=llrm-cc MOS_RT=mos_rt.s LLVM_MOS=sdk-dir python3 tests/asm6502/bench_f64.py [N]` (f64 cycles and bytes vs llvm-mos), `LLRM_CC=llrm-cc tests/run_sim65.sh [n]`, `LLRM_CC=llrm-cc tests/run_sim65_f64.sh [n] [batch]` (binary64 vs the host FPU, standard and edge vectors, per-class counts)
- x86: `tests/size_x86.sh [OPT]`, `tests/size_m16.sh [OPT]`, `tests/x86_kernels.sh`, `tests/x86_kernels_ref.sh`, `tests/x86_m16_kernels.sh [NV] [BENCH_DIR]`

Pricing executables (m68k): `tests/build_m68k_clocks.sh`, `tests/build_m68k_clocks64.sh`, priced with `cycles --exe`.
