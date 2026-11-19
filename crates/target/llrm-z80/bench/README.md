# Z80 bench

The 6502 bench kernels (`kernels.c`, copied from `feat/6502-spike` `tests/bench/kernels.c`, 23 of
them, 16-bit `int`) built by SDCC and run on ucsim's `sz80`. `python3 bench_sdcc.py` writes
`RESULTS-sdcc.md`; needs `sdcc`, `sdasz80`, `ucsim_z80` on PATH (versions are printed in the file).

- Result first: a row counts only if `r` equals the value worked out by hand (table in the script).
- T-states and instructions are the run with the call minus the run without it, so argument
  set-up, call, return and the result store count. No breakpoints: ucsim's `Inst` counter is
  one too high per breakpoint stop (10 NOPs: 12 at the second stop, 10 true).
- Bytes are linked code, library routines included, minus crt0 and the driver.
- Instrument guard: `zero` (2 B, 44 T, 4 insns) and `max8(3,9)` (82 T, 10 insns) are counted by hand
  in the script; it asserts them before writing results.
- ucsim stops with "Stack overflow" on the first `call` unless the initial SP is 0 or 0xFFFF.
