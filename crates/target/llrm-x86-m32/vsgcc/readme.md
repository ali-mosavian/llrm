# crates/target/llrm-x86-m32/vsgcc

llrm's m32 output against gcc and clang: executed instructions, memory operands and a 486 clock
estimate of each `bench/` C kernel in one emulator, and the hottest loop of each. Method, results and
findings: [docs/vs-gcc.md](../../../../docs/vs-gcc.md).

    crates/target/llrm-x86-m32/vsgcc/run.sh                        # builds, runs, prints the tables; products in $VSGCC_WORK
    uv run --project tools python crates/target/llrm-x86-m32/vsgcc/harness.py sieve gccO2   # one program, one compiler
    uv run --project tools python crates/target/llrm-x86-m32/vsgcc/loops.py sieve llrm gccO2 clangO2   # its hottest loop, side by side
    uv run --project tools --group dev pytest crates/target/llrm-x86-m32/vsgcc                # after run.sh

`harness.py` links llrm's OMF itself and gcc/clang's ELF with `ld`, both against `stub.s`, runs `main` in
unicorn and counts `bench_NAME` from entry to its own return. Variants: `llrm`, `gccO2`, `clangO2`, and the
`Os` ones. A rerun with another llrm shows a fix's gain in the table's llrm columns; gcc and clang do not
move.

`kernels/x_*` are 37 more kernels (`bench/` has no copy of them), each with its `.out` self-check from gcc -O0 on the host; every variant
must report the same values or `table.py` refuses. The table prints the bench programs and the kernels as separate summaries, the
worst program beside each geomean.

The kernel is called from `main` through a volatile pointer to it (`wrap.py`, in the copy every compiler builds), so none of them can inline
it into `main`, and none can clone it for `main`'s constants (`-fipa-cp-clone`'s doing in gcc -O3); everything else inlines as the level
says. `-fno-inline-functions` was on for all three before, which the table never showed was turning inlining off.

## Compile-time scaling

`scaling.py` measures how the compile cost of llrm, gcc and clang (`levels_time.py`'s commands, `perf stat instructions:u` and task-clock) grows with program size: six generated axes (functions, straight-line statements, branches, live values, callers, call-chain depth; N doubling until a compile takes 10 s) and QCport's 65 modules against llrm's MIR size (`LLRM_DEBUG=mir`). `run` writes `$VSGCC_WORK/scaling.json`, `report` prints the tables and one PNG per axis. `check` runs each axis' program on gcc, clang and llrm (emulator) and requires one value. Results: [scaling.md](scaling.md). Test: `pytest test_scaling.py` (a stand-in compiler of known quadratic cost must read as slope 2).

    uv run --project tools --with matplotlib python crates/target/llrm-x86-m32/vsgcc/scaling.py run --programs
