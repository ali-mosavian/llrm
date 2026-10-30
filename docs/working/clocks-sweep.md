# Clocks sweep, m16 -Os, main vs parity (2026-10-26)

TL;DR: parity runs 232 of 466 programs slower than main by the 486-priced instruction stream (geomean new/base 0.9957); the dag selector at m16 makes it worse (geomean 1.0167).

Tool: `tools/pairs/clocksweep.py`. Every tests/run, examples and bench program, built for real mode at -Os with its header's other flags, run whole
(start-up and runtime included) in `icount.py` (unicorn), priced by vsgcc's 486 table. Output must equal NAME.out on both arms or the program is void.
Arms: base = `sbp-854bb7efc` (main), new = `sbp-d6d94ee7b0` (feat/parity d6d94ee7b0). Full lists: `default.txt`, `dag.txt` beside this file.

## Instrument

- Output checked per program; 27 void (did not build, run or print the expected output on one side).
- `tiny` = under twice the least count of its language: bench/parity/* kernels (a few hundred clocks) are start-up, so their % says nothing.
- Against DOSBox (RDTSC cycles of the whole program, fixed-cycle clock), four programs, new/base ratio:

| program | emulator instructions | DOSBox cycles | emulator 486 clocks |
|---|---|---|---|
| fib.bas | 0.9431 | 0.9436 | 0.9905 |
| nbody_fixed.nib | 1.4322 | 1.4322 | 1.3998 |
| quicksort.nib | 1.5086 | 1.5057 | 1.2419 |
| sieve.c | 1.3876 | 1.3676 | 1.2246 |

  The emulator's instruction counts agree with DOSBox within 0.05-1.5% (the larger on the smallest program). DOSBox charges about one cycle per instruction, so
  it cannot validate the clock prices: its ms follows instruction counts and overstates a slowdown whose instructions are cheap (quicksort.nib 1.51 vs 1.24). The clock
  model is vsgcc's table, validated by its own tests, not by DOSBox.

## Per language (geomean new/base, programs slower)

| source | default isel (old at m16) | `--isel=dag` |
|---|---|---|
| .bas (185) | 0.9964, 106 slower | 1.0219, 131 slower |
| .nib (107) | 0.9818, 52 slower | 0.9711, 33 slower |
| .c (174) | 1.0035, 74 slower | 1.0402, 85 slower |

## Slowest, default isel (not tiny)

```
  +513.70%      +26928 clocks  (5242 -> 32170)  c/stackover.c [-Os -fsanitize=stack]
```

## Slowest, `--isel=dag` (not tiny)

```
  +405.32%      +21247 clocks  (5242 -> 26489)  c/stackover.c [-Os -fsanitize=stack]
  +126.22%     +907559 clocks  (719019 -> 1626578)  scroll/scroll.c [-Os -march=i486]
```
