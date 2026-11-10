# m68k executed clocks: ours against the best of gcc and clang (2026-11-10 19:35Z, m68k-b)

TL;DR: 1112 cells (139 tests/run/c programs x 68000, 68020 x O1 O2 O3 Os); ours is faster or equal in 537 (497 better, 40 equal), slower than the best external in 468 (271 by more than 5%, 84 by more than 50%), UNMEASURED in 107 (71 no external number, 36 ours). Clocks only: the O2/O3 rate and Os (bytes) verdicts need the byte column.

Method: release/next c98e231ab163; md5 llrm-cc 589675fe, llrm-llc ab16db62, llrm-m68k-llc 4eec43b1, cycles 061c05b0. One exe per cell and column (ours: `llrm-cc -c`; gcc 15 `-msoft-float`; clang: pin clang into pin llc, shared external flags), linked with our compiler-rt archive and the harness stubs. `cycles --exe`, CYCLES_CPUS set to the exe's own CPU (the tool prints the 68000 column first: a first 68020 pass read it, caught by a raw look). Every exe also runs under qemu and its stdout is compared to the reference; a wrong run is UNMEASURED, not a number (the first table showed externals 2000x faster, 123 clang runs were wrong). Table: m68k-clocks-2026-11-10.tsv, one row per cell.

## Counts (better / equal / worse / UNMEASURED ours / UNMEASURED no external)

68000 O1: better 66, equal 2, worse 62, UNMEASURED(ours) 2, UNMEASURED(no external) 7
68000 O2: better 68, equal 7, worse 56, UNMEASURED(ours) 2, UNMEASURED(no external) 6
68000 O3: better 60, equal 7, worse 64, UNMEASURED(ours) 2, UNMEASURED(no external) 6
68000 Os: better 65, equal 12, worse 53, UNMEASURED(ours) 2, UNMEASURED(no external) 7
68020 O1: better 63, equal 3, worse 53, UNMEASURED(ours) 7, UNMEASURED(no external) 13
68020 O2: better 69, equal 3, worse 49, UNMEASURED(ours) 7, UNMEASURED(no external) 11
68020 O3: better 62, equal 4, worse 57, UNMEASURED(ours) 7, UNMEASURED(no external) 9
68020 Os: better 44, equal 2, worse 74, UNMEASURED(ours) 7, UNMEASURED(no external) 12

## Findings

- float_in_memory_loop: ours 3.54 M clocks (68000 O2), clang 1798, gcc 5128. The loop adds 16 x 0.5 in memory 100 times; both externals fold it, ours runs 1600 soft-float adds. Largest gap in the table; the others over 50% are listed in the tsv (int64_division, float_in_place_update, urem_eq, uint64_to_float, loop_versioning, byte_multiply, idioms, word_compare_branches, select_of_constants, idiv_loop_os, ...).
- os_console: ours and the externals differ from the reference output in this harness (stdin/args); UNMEASURED at all levels.
- cycles timeouts (300 s) on memfuncs_runtime and shl_of_a_zero_extended_dividend (ours); UNMEASURED.

## Instrument gaps (UNMEASURED cells, not compiler findings)

- The 68020 timing tables lack `trap #0` (the harness syscall; same count in every column of a program, so cells whose only unpriced instruction is the trap are kept, as a common lower bound), `mulu.l`/`muls.l #imm`, `cmpi.l #imm,(0,%pc)` and a full-extension indexed form. Owner: the cycles/timing session.
- External columns missing: gcc 28 abi-mismatch (long double), 48 compile, 4 link, 4 no-gcc-column, 4 cycles; clang 46 compile failures and 123 wrong runs.
