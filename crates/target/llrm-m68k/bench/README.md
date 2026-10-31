# m68k bench

Twelve leaf kernels (`kernels/*.ll`), each compiled by `llrm-m68k` and by the pin's `llc`
(`-O2`, and `-O2` with `optsize`), run under `qemu-m68k -cpu m68000` from one `_start`, with
the same arguments and data. `cargo run -p llrm-m68k --example bench` prints the table.

- **Sizes.** `int`, `long`, pointers and `size_t` are 32 bits; the datalayout is the pin's
  (`E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16`). Checked by
  `tests/kernels.rs::clang_agrees_with_the_target_on_sizes_and_datalayout` (clang with
  `--target=m68k-linux-gnu`).
- **Results first.** A row counts only if every variant exits with the value worked out by hand
  (`kernels::table`). `tests/kernels.rs` checks the same for ours, bounded by `timeout 10`.
- **Instructions** are executed instructions inside `k`, counted from qemu's trace through a
  pipe (never a file), killed at 2M lines. Cycles are not modelled.
- **gcc.** `kernels/*.c` mirror the IR; compiled with `m68k-linux-gnu-gcc -Os` and `-O2` from `$GCC_M68K` (default
  `~/scratch/gcc-m68k/root`, the apt debs unpacked without root); the columns read `-` when it is missing.
- **Not compared.** vbcc.
- The 68000 model matters: `llc` saves and restores `sr` around copies (`move.w %sr,-(%sp)`),
  which faults on a 68010 or later model.
